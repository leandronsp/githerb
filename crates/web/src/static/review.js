/* githerb — the review surface.
   The browser holds the selection, the theme and nothing else. Everything on
   the page came from the server and goes back to it; a page that disagrees
   with the repository is a bug in the stream, never a second copy of state. */

(() => {
  "use strict";

  const body = document.body;
  const proposal = body.dataset.proposal;
  const find = (selector, root) => (root || document).querySelector(selector);

  // --- saying things ---

  let hush;
  const say = (text) => {
    const out = find("#toast");
    if (!out) return;
    out.textContent = text;
    out.hidden = false;
    clearTimeout(hush);
    hush = setTimeout(() => {
      out.hidden = true;
    }, 4000);
  };

  const post = async (path, fields) => {
    let answer;
    try {
      answer = await fetch(path, {
        method: "POST",
        headers: { "content-type": "application/x-www-form-urlencoded" },
        body: new URLSearchParams(fields || {}),
      });
    } catch (error) {
      say("the server is not answering");
      return false;
    }
    if (answer.ok) return true;
    say((await answer.text()) || "refused");
    return false;
  };

  const theme = () => {
    const root = document.documentElement;
    const dark = root.dataset.theme
      ? root.dataset.theme === "dark"
      : matchMedia("(prefers-color-scheme: dark)").matches;
    root.dataset.theme = dark ? "light" : "dark";
    try {
      localStorage.setItem("githerb:theme", root.dataset.theme);
    } catch (error) {
      /* a browser that refuses storage still gets the theme for this page */
    }
  };

  const jump = (target) => {
    if (!target) return;
    target.scrollIntoView({ block: "center", behavior: "smooth" });
    target.classList.remove("flash");
    void target.offsetWidth;
    target.classList.add("flash");
    setTimeout(() => target.classList.remove("flash"), 600);
  };

  // --- the selection ---

  let pick = null;
  let anchor = null;
  let dragging = false;
  let painted = [];
  let replying = null;

  const numberOf = (row, side) => {
    if (!row.cells || row.cells.length < 3) return 0;
    return parseInt(row.cells[side === "old" ? 0 : 1].textContent, 10) || 0;
  };

  const spotOf = (target) => {
    if (!target || !target.closest) return null;
    const cell = target.closest("td.o, td.n");
    if (!cell) return null;
    const line = parseInt(cell.textContent, 10);
    const file = cell.closest("section.file");
    if (!line || !file) return null;
    return {
      file: file.dataset.path,
      side: cell.classList.contains("o") ? "old" : "new",
      line,
      row: cell.parentElement,
    };
  };

  const between = (from, side, start, end) => {
    const rows = [];
    for (let row = from; row; row = row.previousElementSibling) {
      const line = numberOf(row, side);
      if (!line) continue;
      if (line < start) break;
      if (line <= end) rows.unshift(row);
    }
    for (let row = from.nextElementSibling; row; row = row.nextElementSibling) {
      const line = numberOf(row, side);
      if (!line) continue;
      if (line > end) break;
      if (line >= start) rows.push(row);
    }
    return rows;
  };

  const paint = () => {
    painted.forEach((row) => row.classList.remove("picked"));
    painted = pick ? between(pick.row, pick.side, pick.start, pick.end) : [];
    painted.forEach((row) => row.classList.add("picked"));
  };

  const drop = () => {
    pick = null;
    anchor = null;
    paint();
    close();
  };

  // --- the composer ---

  const close = () => {
    document.querySelectorAll(".composer-row, .thread form").forEach((node) => {
      const row = node.closest(".composer-row");
      (row || node).remove();
    });
    replying = null;
  };

  const form = (label, placeholder, where) => {
    const template = find("#composer");
    const built = template.content.firstElementChild.cloneNode(true);
    const box = find("textarea", built);
    box.placeholder = placeholder;
    find(".where", built).textContent = where;
    find("button[type=submit]", built).textContent = label;
    return built;
  };

  const compose = () => {
    if (!pick) return;
    close();
    const span = pick.start === pick.end ? pick.start : pick.start + "–" + pick.end;
    const built = form("Leave note", "what needs to change here?", pick.file + ":" + span + " " + pick.side);
    const row = document.createElement("tr");
    const cell = document.createElement("td");
    row.className = "composer-row";
    cell.colSpan = 3;
    cell.appendChild(built);
    row.appendChild(cell);
    (painted[painted.length - 1] || pick.row).after(row);
    find("textarea", built).focus();
  };

  const answer = (note) => {
    const thread = document.getElementById("t-" + note);
    if (!thread) return;
    close();
    replying = note;
    const who = find(".who", thread);
    const built = form("Reply", "what do you want to say back?", "answering " + (who ? who.textContent : note));
    find(".doing", thread).after(built);
    find("textarea", built).focus();
  };

  // --- pointer ---

  document.addEventListener("mousedown", (event) => {
    const spot = spotOf(event.target);
    if (!spot) return;
    event.preventDefault();
    if (event.shiftKey && anchor && anchor.file === spot.file && anchor.side === spot.side) {
      pick = {
        file: spot.file,
        side: spot.side,
        start: Math.min(anchor.line, spot.line),
        end: Math.max(anchor.line, spot.line),
        row: anchor.row,
      };
    } else {
      anchor = spot;
      pick = { file: spot.file, side: spot.side, start: spot.line, end: spot.line, row: spot.row };
    }
    dragging = true;
    paint();
  });

  document.addEventListener("mouseover", (event) => {
    if (!dragging || !anchor) return;
    const spot = spotOf(event.target);
    if (!spot || spot.file !== anchor.file || spot.side !== anchor.side) return;
    pick = {
      file: anchor.file,
      side: anchor.side,
      start: Math.min(anchor.line, spot.line),
      end: Math.max(anchor.line, spot.line),
      row: anchor.row,
    };
    paint();
  });

  document.addEventListener("mouseup", () => {
    if (!dragging) return;
    dragging = false;
    if (pick) compose();
  });

  // --- clicks ---

  document.addEventListener("click", (event) => {
    const target = event.target;
    if (!target.closest) return;

    const hit = (name) => target.closest("[" + name + "]");
    const cancel = hit("data-cancel");
    if (cancel) return drop();

    const reply = hit("data-reply");
    if (reply) return answer(reply.dataset.reply);

    const resolve = hit("data-resolve");
    if (resolve) {
      post("/p/" + proposal + "/resolve", { note: resolve.dataset.resolve });
      return;
    }

    if (hit("data-dispatch")) {
      post("/p/" + proposal + "/dispatch").then((ok) => ok && say("handed to the agent"));
      return;
    }

    const land = hit("data-land");
    if (land) {
      if (land.disabled) return;
      post("/p/" + proposal + "/land").then((ok) => ok && say("landed"));
      return;
    }

    if (hit("data-abandon")) {
      if (confirm("Abandon this proposal?")) post("/p/" + proposal + "/abandon");
      return;
    }

    const handover = hit("data-handover");
    if (handover) {
      event.preventDefault();
      fetch(handover.getAttribute("href"))
        .then((got) => got.text())
        .then((text) => navigator.clipboard.writeText(text))
        .then(() => say("handover copied"))
        .catch(() => say("could not copy the handover"));
      return;
    }

    if (hit("data-theme")) return theme();

    const fold = target.closest(".fold");
    if (fold) {
      const file = fold.closest(".file");
      const open = file.classList.toggle("folded");
      fold.setAttribute("aria-expanded", open ? "false" : "true");
      return;
    }

    const load = target.closest(".load");
    if (load) {
      const where = "/p/" + proposal + "/file/" + load.dataset.file + location.search;
      fetch(where)
        .then((got) => got.text())
        .then((html) => load.outerHTML = html)
        .catch(() => say("could not load that file"));
      return;
    }

    const anchored = target.closest('a[href^="#"]');
    if (anchored) {
      const found = document.getElementById(anchored.getAttribute("href").slice(1));
      if (found) {
        event.preventDefault();
        jump(found);
      }
    }
  });

  // --- forms ---

  document.addEventListener("submit", (event) => {
    event.preventDefault();
    const said = find("textarea", event.target).value.trim();
    if (!said) return;
    const done = (ok) => ok && (replying ? close() : drop());
    if (replying) post("/p/" + proposal + "/replies", { note: replying, body: said }).then(done);
    else if (pick)
      post("/p/" + proposal + "/comments", {
        file: pick.file,
        side: pick.side,
        start: pick.start,
        end: pick.end,
        body: said,
      }).then(done);
  });

  // --- keyboard ---

  const step = (selector, back) => {
    const all = Array.from(document.querySelectorAll(selector));
    if (!all.length) return;
    const middle = window.innerHeight / 2;
    const next = back
      ? all.reverse().find((node) => node.getBoundingClientRect().top < middle - 4)
      : all.find((node) => node.getBoundingClientRect().top > middle + 4);
    jump(next || all[0]);
  };

  document.addEventListener("keydown", (event) => {
    const typing = /^(INPUT|TEXTAREA|SELECT)$/.test(event.target.tagName);
    if (typing) {
      if (event.key === "Escape") drop();
      if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) {
        event.target.closest("form").requestSubmit();
      }
      return;
    }
    if (event.metaKey || event.ctrlKey || event.altKey) return;
    switch (event.key) {
      case "n": step(".thread-row", false); break;
      case "p": step(".thread-row", true); break;
      case "]": step(".file", false); break;
      case "[": step(".file", true); break;
      case "c": compose(); break;
      case "Escape": drop(); break;
      default: return;
    }
    event.preventDefault();
  });

  // --- the stream ---

  let stream = null;

  const upsert = (thread) => {
    const already = document.getElementById(thread.id);
    if (already) {
      // A half-typed answer outlives an update that arrived under it.
      if (find("form", already)) return;
      already.outerHTML = thread.html;
      return;
    }
    const after = document.getElementById(thread.after);
    if (after) after.insertAdjacentHTML("afterend", thread.html);
  };

  const apply = (update) => {
    const bar = find("#bar");
    const rail = find("#rail");
    if (bar && update.bar) bar.outerHTML = update.bar;
    if (rail && update.rail) rail.outerHTML = update.rail;
    const alive = new Set();
    (update.threads || []).forEach((thread) => {
      alive.add(thread.id);
      upsert(thread);
    });
    (update.removed || []).forEach((gone) => {
      const node = document.getElementById(gone);
      if (node) node.remove();
    });
    document.querySelectorAll(".thread-row").forEach((row) => {
      if (!alive.has(row.id)) row.remove();
    });
    body.dataset.fp = update.fp;
  };

  const listen = () => {
    const query = proposal
      ? "/p/" + proposal + "/events" + (location.search ? location.search + "&" : "?") + "fp=" + body.dataset.fp
      : "/events";
    stream = new EventSource(query);
    stream.addEventListener("update", (event) => apply(JSON.parse(event.data)));
    stream.addEventListener("revision", () => location.reload());
    stream.addEventListener("board", (event) => {
      const board = find("#board");
      if (board) board.outerHTML = event.data;
    });
    stream.onopen = () => find("#bar").classList.remove("offline");
    stream.onerror = () => {
      find("#bar").classList.add("offline");
      stream.close();
      setTimeout(listen, 2000);
    };
  };

  addEventListener("pagehide", () => stream && stream.close());
  listen();
})();
