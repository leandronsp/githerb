#!/usr/bin/env bash
# The whole product against a real repository: propose from the terminal,
# annotate in a browser, watch the panel move when an agent answers from the
# terminal, then land from the browser.
set -uo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN=$ROOT/bin/githerb
PORT=${PORT:-4278}
WORK=$(mktemp -d)
WEB="http://127.0.0.1:$PORT"

PASSED=0
FAILED=0
REPORT=()
STARTED=$SECONDS

cleanup() {
  [ -n "${SERVER:-}" ] && kill "$SERVER" 2>/dev/null
  rm -rf "$WORK"
}
trap cleanup EXIT

step() {
  local name=$1 detail=${2:-}
  shift 2
  local began=$SECONDS
  if "$@" >"$WORK/out" 2>&1; then
    REPORT+=("$(printf '  %-30s ok    %2ss  %s' "$name" "$((SECONDS - began))" "$detail")")
    PASSED=$((PASSED + 1))
  else
    REPORT+=("$(printf '  %-30s FAIL  %2ss  %s' "$name" "$((SECONDS - began))" "$(tail -2 "$WORK/out" | tr '\n' ' ')")")
    FAILED=$((FAILED + 1))
  fi
}

jsq() { printf '%s' "$1" | python3 -c 'import json,sys; print(json.dumps(sys.stdin.read()))'; }
js()  { agent-browser eval "$1" 2>/dev/null | tr -d '"'; }
click() { agent-browser eval "(() => { const e = document.querySelector($(jsq "$1")); if (!e) return 'missing'; e.dispatchEvent(new MouseEvent('click', {bubbles:true, shiftKey:${2:-false}})); return 'ok'; })()" 2>/dev/null | grep -q ok; }

# Picking lines happens in the gutter on mousedown, the way it does in every
# other diff, so the smoke has to press there rather than click the code.
gutter() { agent-browser eval "(() => { const e = document.querySelector($(jsq "$1") + ' .no.new'); if (!e) return 'missing'; e.dispatchEvent(new MouseEvent('mousedown', {bubbles:true, shiftKey:${2:-false}})); e.dispatchEvent(new MouseEvent('mouseup', {bubbles:true})); return 'ok'; })()" 2>/dev/null | grep -q ok; }

## the repository

do_repo() {
  cd "$WORK" || return 1
  git init -q -b main && git config user.email smoke@githerb && git config user.name smoke
  printf 'one\ntwo\nthree\nfour\n' > a.txt
  git add . && git commit -qm root
  git checkout -q -b work
  printf 'one\nTWO\ntwo and a half\nthree\nfour\n' > a.txt
  git commit -qam "the work"
}

do_propose() {
  cd "$WORK" || return 1
  "$BIN" propose --onto main --title "Rewrite the reader" || return 1
  ID=$("$BIN" list | awk '{print $1}')
  [ -n "$ID" ]
}

do_serve() {
  cd "$WORK" || return 1
  "$BIN" review --port "$PORT" --open=false >"$WORK/server.log" 2>&1 &
  SERVER=$!
  for _ in $(seq 1 40); do
    curl -sf -o /dev/null "$WEB/p/$ID" && return 0
    sleep 0.25
  done
  return 1
}

## the browser

do_open() {
  agent-browser cookies clear >/dev/null 2>&1
  agent-browser open "$WEB/p/$ID" >/dev/null 2>&1 || return 1
  sleep 1
  [ "$(js 'document.querySelectorAll(".line").length > 0')" = "true" ]
}

do_select() {
  gutter '.line[data-file="a.txt"][data-line="new:2"]' false || return 1
  gutter '.line[data-file="a.txt"][data-line="new:3"]' true || return 1
  sleep 0.4
  [ "$(js 'document.querySelectorAll(".line.picked").length')" = "2" ]
}

do_annotate() {
  agent-browser eval "(() => {
    const t = document.querySelector('textarea');
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(t, 'these two want a name');
    t.dispatchEvent(new Event('input', {bubbles:true}));
    return 'ok';
  })()" >/dev/null 2>&1 || return 1
  sleep 0.3
  click '.composer button' false || return 1
  sleep 1.5
  js 'document.querySelector("#panel").innerText' | grep -q "these two want a name"
}

do_land_blocked() {
  [ "$(js 'document.querySelector(".land").disabled')" = "true" ]
}

# The header has to say what the diff did before anyone reads a line of it.
do_counts() {
  [ "$(js 'document.querySelector("#page .meta .added").innerText')" = "+2" ] || return 1
  [ "$(js 'document.querySelector("#page .meta .removed").innerText.length')" = "2" ]
}

# A file folds away when it is not the one being read, and unfolds again.
do_fold() {
  click '.file > h2' false || return 1
  sleep 0.3
  [ "$(js 'document.querySelector(".file").classList.contains("folded")')" = "true" ] || return 1
  [ "$(js 'document.querySelector(".hunk").offsetParent === null')" = "true" ] || return 1

  click '.file > h2' false || return 1
  sleep 0.3
  [ "$(js 'document.querySelector(".hunk").offsetParent !== null')" = "true" ] || return 1

  click '[data-fold-all]' false || return 1
  sleep 0.3
  [ "$(js 'document.querySelectorAll(".file.folded").length === document.querySelectorAll(".file").length')" = "true" ] || return 1

  click '[data-unfold-all]' false || return 1
  sleep 0.3
  [ "$(js 'document.querySelectorAll(".file.folded").length')" = "0" ]
}

# The board says how big a proposal is before anyone opens it.
do_board() {
  curl -sf "$WEB/" | tr -d '\n' | grep -q "+2"
}

# One button hands the whole review over, the way an annotation buffer does.
# A headless browser has no clipboard permission, so the fallback path is the
# one under test here: either way the brief has to be somewhere to paste from.
do_handover() {
  curl -sf "$WEB/p/$ID/handover" | grep -q "these two want a name" || return 1

  click '[data-handover]' false || return 1
  sleep 1
  [ "$(js 'document.querySelector("[data-handover]").dataset.handedOver')" = "1" ]
}

# The reactive proof: the browser is not touched, an agent answers from the
# terminal, and the panel has to move on its own.
do_live_update() {
  [ "$(js 'document.querySelector("#panel .count").innerText')" = "1" ] || return 1

  cd "$WORK" || return 1
  local comment
  comment=$("$BIN" comments "$ID" | awk '{print $1}')
  [ -n "$comment" ] || return 1

  GITHERB_AUTHOR=claude-code "$BIN" resolve "$ID" "$comment" || return 1

  for _ in $(seq 1 20); do
    [ "$(js 'document.querySelector("#panel .count").innerText')" = "0" ] && return 0
    sleep 0.5
  done
  return 1
}

# A new revision moves the lines, so the whole page has to come back, not only
# the panel. The mark survives a swap and dies on a reload, which is how this
# tells the two apart.
do_new_revision() {
  agent-browser eval "window.__mark = 'held'" >/dev/null 2>&1
  [ "$(js 'document.querySelector("#page .meta").innerText.includes("revision 1")')" = "true" ] || return 1

  cd "$WORK" || return 1
  printf 'one\nTWO_NAMED\ntwo and a half\nthree\nfour\n' > a.txt
  git commit -qam "the fix"
  "$BIN" revise "$ID" >/dev/null || return 1

  for _ in $(seq 1 20); do
    if [ "$(js 'document.querySelector("#page .meta").innerText.includes("revision 2")')" = "true" ]; then
      [ "$(js 'window.__mark')" = "held" ] || return 1
      [ "$(js 'document.querySelector(".scope") !== null')" = "true" ]
      return $?
    fi
    sleep 0.5
  done
  return 1
}

# Every navigation opens an event stream, and a browser gives one host six
# connections, so a stream left open across a click starves the page the next
# click asks for. The symptom is a tab that hangs while the server is idle, so
# this measures from inside the page rather than with curl.
do_streams() {
  for _ in $(seq 1 8); do
    agent-browser eval "(() => { const l = [...document.querySelectorAll('.scope a')]; if (!l.length) return 'none'; l[0].click(); return 'ok'; })()" >/dev/null 2>&1
    sleep 0.6
  done

  # What the browser is starved of is connections, so count them rather than
  # time the symptom: the page and one stream, never a stream per click.
  local held
  held=$(lsof -nP -p "$SERVER" 2>/dev/null | grep -c ESTABLISHED)
  [ "$held" -le 3 ]
}

do_land_from_browser() {
  [ "$(js 'document.querySelector(".land").disabled')" = "false" ] || return 1
  click '.land' false || return 1
  sleep 1.5
  cd "$WORK" || return 1
  [ "$(git rev-parse main)" = "$(git rev-parse work)" ]
}

command -v agent-browser >/dev/null || { echo "smoke needs agent-browser on PATH"; exit 1; }

echo
echo "githerb smoke  ·  $WEB"
echo

step "a repository with work"      ""  do_repo
step "propose from the terminal"   ""  do_propose
step "the review surface answers"  ""  do_serve
step "the diff renders"            ""  do_open
step "shift-click picks a range"   ""  do_select
step "annotate in the browser"     ""  do_annotate
step "landing is blocked"          ""  do_land_blocked
step "the header counts the diff"  ""  do_counts
step "a file folds away"           ""  do_fold
step "the board sizes each one"    ""  do_board
step "hand the review over"        ""  do_handover
step "the panel moves on its own"  ""  do_live_update
step "a new revision redraws it"   ""  do_new_revision
step "clicking around stays fast"  ""  do_streams
step "land from the browser"       ""  do_land_from_browser

printf '%s\n' "${REPORT[@]}"
echo
if [ "$FAILED" -eq 0 ]; then
  echo "  $PASSED passed in $((SECONDS - STARTED))s"
else
  echo "  $PASSED passed, $FAILED failed in $((SECONDS - STARTED))s"
  exit 1
fi
