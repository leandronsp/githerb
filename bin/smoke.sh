#!/usr/bin/env bash
# The whole product against a real repository: propose from the terminal,
# annotate in a browser, watch the page move when an agent answers from the
# terminal, then land from the browser.
set -uo pipefail

ROOT=$(cd "$(dirname "$0")/.." && pwd)
BIN=${BIN:-$ROOT/target/release/githerb}
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
# other diff, so the smoke presses the new-side number cell of that line.
gutter() {
  agent-browser eval "(() => {
    const section = document.querySelector('section.file[data-path=\"' + $(jsq "$1") + '\"]');
    if (!section) return 'missing';
    const cell = [...section.querySelectorAll('td.n')].find(td => td.textContent.trim() === '$2');
    if (!cell) return 'missing';
    cell.dispatchEvent(new MouseEvent('mousedown', {bubbles:true, shiftKey:${3:-false}}));
    cell.dispatchEvent(new MouseEvent('mouseup', {bubbles:true}));
    return 'ok';
  })()" 2>/dev/null | grep -q ok
}

# Type into the one textarea the page has open and send it.
say() {
  agent-browser eval "(() => {
    const t = document.querySelector('textarea');
    if (!t) return 'missing';
    Object.getOwnPropertyDescriptor(HTMLTextAreaElement.prototype,'value').set.call(t, $(jsq "$1"));
    t.dispatchEvent(new Event('input', {bubbles:true}));
    return 'ok';
  })()" 2>/dev/null | grep -q ok || return 1
  sleep 0.2
  click 'form button[type="submit"]' false
}

# Poll the page until a JS expression reads true, or give up.
until_js() {
  local tries=${2:-20}
  for _ in $(seq 1 "$tries"); do
    [ "$(js "$1")" = "true" ] && return 0
    sleep 0.5
  done
  return 1
}

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
  "$BIN" review --port "$PORT" --no-open >"$WORK/server.log" 2>&1 &
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
  until_js 'document.querySelectorAll("#diff tr[id^=\"L-\"]").length > 0' 10
}

do_select() {
  gutter a.txt 2 false || return 1
  gutter a.txt 3 true || return 1
  until_js 'document.querySelectorAll("tr.picked").length === 2' 4
}

do_annotate() {
  say 'these two want a name' || return 1
  until_js 'document.querySelector("#rail .threads") !== null && document.querySelector("#rail .threads").innerText.includes("these two want a name")'
}

# A note is a thread: it renders where the code is, and anyone can answer it.
do_thread() {
  [ "$(js 'document.querySelectorAll("tr.thread-row").length')" = "1" ] || return 1
  js 'document.querySelector("tr.thread-row").innerText' | grep -q "these two want a name" || return 1
  # It sits right after the last line it covers.
  [ "$(js 'document.querySelector("tr.thread-row").previousElementSibling.querySelector("td.n").textContent.trim()')" = "3" ] || return 1

  click 'tr.thread-row [data-reply]' false || return 1
  sleep 0.3
  say 'naming both of them costs nothing' || return 1
  until_js 'document.querySelector("tr.thread-row .answer") !== null && document.querySelector("tr.thread-row .answer").innerText.includes("naming both of them costs nothing")'
}

do_land_blocked() {
  [ "$(js 'document.querySelector("[data-land]").disabled')" = "true" ]
}

# The bar has to say what the diff did before anyone reads a line of it.
do_counts() {
  [ "$(js 'document.querySelector("#bar .added").innerText')" = "+2" ] || return 1
  [ "$(js 'document.querySelector("#bar .removed").innerText.length')" = "2" ]
}

# A file folds away when it is not the one being read, and unfolds again.
do_fold() {
  click 'section.file .fold' false || return 1
  sleep 0.3
  [ "$(js 'document.querySelector("section.file").classList.contains("folded")')" = "true" ] || return 1
  [ "$(js 'document.querySelector("section.file table").offsetParent === null')" = "true" ] || return 1
  click 'section.file .fold' false || return 1
  sleep 0.3
  [ "$(js 'document.querySelector("section.file table").offsetParent !== null')" = "true" ]
}

# The board says how big a proposal is before anyone opens it.
do_board() {
  curl -sf "$WEB/" | tr -d '\n' | grep -q "+2"
}

# The brief is one request away, and one button hands the notes to the agent.
do_handover() {
  curl -sf "$WEB/p/$ID/handover" | grep -q "these two want a name" || return 1
  click '[data-dispatch]' false || return 1
  cd "$WORK" || return 1
  for _ in $(seq 1 10); do
    # The runner alongside may already have claimed the handover; either way the
    # ask is in the log and the agent line says so.
    "$BIN" show "$ID" | grep -Eq "waiting for an agent|is apply|apply failed" && return 0
    sleep 0.4
  done
  return 1
}

# The reactive proof: the browser is not touched, somebody answers from the
# terminal, and the rail has to move on its own.
do_live_update() {
  [ "$(js 'document.querySelectorAll("#rail ul.threads:not(.done) > li").length')" = "1" ] || return 1
  cd "$WORK" || return 1
  local comment
  comment=$("$BIN" comments "$ID" | awk '{print $1}')
  [ -n "$comment" ] || return 1
  GITHERB_AUTHOR=claude-code "$BIN" resolve "$ID" "$comment" || return 1
  until_js 'document.querySelectorAll("#rail ul.threads:not(.done) > li").length === 0'
}

# A new revision moves the lines, so the page comes back whole and says so.
do_new_revision() {
  [ "$(js 'document.querySelector("#bar").innerText.includes("r1")')" = "true" ] || return 1
  cd "$WORK" || return 1
  printf 'one\nTWO_NAMED\ntwo and a half\nthree\nfour\n' > a.txt
  git commit -qam "the fix"
  "$BIN" revise "$ID" >/dev/null || return 1
  until_js '/\br2\b/.test(document.querySelector("#bar").innerText)' || return 1
  [ "$(js 'document.querySelector("#bar .origins") !== null')" = "true" ]
}

# The other half of the product: a note handed over, an agent answering it in a
# worktree, and the page saying so without being touched.
do_agent() {
  cd "$WORK" || return 1
  cat > .githerb.toml <<'TOML'
[agent]
command = "cat > brief.txt && printf 'one\nTWO_BY_AGENT\ntwo and a half\nthree\nfour\n' > a.txt && git add -A && git commit -qm 'the agent answered'"
TOML
  "$BIN" comment "$ID" --file a.txt --line 2 --body "the agent should name this" >/dev/null || return 1
  "$BIN" dispatch "$ID" >/dev/null || return 1

  local before after
  before=$("$BIN" show "$ID" | awk '/^state/ {print $4}')
  after=$before
  # Nothing is started here on purpose: the review surface carries the runner,
  # so handing the notes over is the whole trigger.
  for _ in $(seq 1 40); do
    after=$("$BIN" show "$ID" | awk '/^state/ {print $4}')
    [ "$after" -gt "$before" ] && break
    sleep 0.5
  done
  [ "$after" -gt "$before" ] || return 1
  # The agent works in its own worktree, so the checkout here never moved.
  grep -q TWO_NAMED a.txt || return 1
  until_js "/\\br${after}\\b/.test(document.querySelector('#bar').innerText)" || return 1
  until_js 'document.querySelector("#bar .agent").innerText.includes("no agent on it")' 10
}

# Every navigation opens an event stream, and a browser gives one host six
# connections, so a stream left open across a click starves the next page.
do_streams() {
  for _ in $(seq 1 8); do
    agent-browser eval "(() => { const l = [...document.querySelectorAll('#bar .origins a')]; if (!l.length) return 'none'; l[0].click(); return 'ok'; })()" >/dev/null 2>&1
    sleep 0.6
  done
  local held
  held=$(lsof -nP -p "$SERVER" 2>/dev/null | grep -c ESTABLISHED)
  [ "$held" -le 3 ]
}

do_land_from_browser() {
  if [ "$(js 'document.querySelector("[data-land]").disabled')" != "false" ]; then
    echo "blocked: $(js 'document.querySelector("#bar").innerText' | tr "\n" " ")"
    cd "$WORK" && "$BIN" show "$ID" | head -20
    return 1
  fi
  click '[data-land]' false || return 1
  cd "$WORK" || return 1
  local head
  head=$("$BIN" show "$ID" | awk '/^  r/ {sha=$2} END {print sha}')
  for _ in $(seq 1 10); do
    [ "$(git rev-parse --short main)" = "$head" ] && return 0
    sleep 0.5
  done
  return 1
}

command -v agent-browser >/dev/null || { echo "smoke needs agent-browser on PATH"; exit 1; }
[ -x "$BIN" ] || { echo "smoke needs the binary at $BIN (make build)"; exit 1; }

echo
echo "githerb smoke  ·  $WEB"
echo

step "a repository with work"      ""  do_repo
step "propose from the terminal"   ""  do_propose
step "the review surface answers"  ""  do_serve
step "the diff renders"            ""  do_open
step "shift-click picks a range"   ""  do_select
step "annotate in the browser"     ""  do_annotate
step "a note is a thread"          ""  do_thread
step "landing is blocked"          ""  do_land_blocked
step "the bar counts the diff"     ""  do_counts
step "a file folds away"           ""  do_fold
step "the board sizes each one"    ""  do_board
step "hand the review over"        ""  do_handover
step "the rail moves on its own"   ""  do_live_update
step "a new revision redraws it"   ""  do_new_revision
step "an agent answers a handover" ""  do_agent
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
