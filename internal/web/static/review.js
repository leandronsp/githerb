// The whole client. It knows which lines are picked and nothing else: every
// other piece of state is rendered by the server and arrives over the event
// stream, which is what keeps the two from ever disagreeing.
(() => {
  const proposal = document.body.dataset.proposal;
  const composer = document.querySelector(".composer");
  const box = composer.querySelector("textarea");
  const where = composer.querySelector(".where");
  const send = composer.querySelector("button");

  const said = document.querySelector(".said");
  let saying = 0;

  // The bar and the panel are replaced whenever the log moves, so anything the
  // page wants to say has to live outside both of them.
  const say = (text) => {
    said.textContent = text;
    said.hidden = false;

    clearTimeout(saying);
    saying = setTimeout(() => (said.hidden = true), 4000);
  };

  let pick = null;
  let anchor = null;

  // The note being answered. A reply belongs to a thread, not to a range of
  // lines, so it is the one selection that has nothing to do with the gutter.
  let answering = null;

  const paint = () => {
    for (const line of document.querySelectorAll(".line")) {
      const on =
        pick &&
        line.dataset.file === pick.file &&
        line.dataset.side === pick.side &&
        Number(line.dataset.no) >= pick.start &&
        Number(line.dataset.no) <= pick.end;
      line.classList.toggle("picked", Boolean(on));
    }

    composer.hidden = !pick && !answering;
    if (!pick || answering) return;

    // Park the composer under the last line it is about. The eye does not have
    // to travel, which is the whole point of writing the note here.
    const last = document.querySelector(
      `.line[data-file="${CSS.escape(pick.file)}"][data-side="${pick.side}"][data-no="${pick.end}"]`,
    );
    last?.after(composer);

    const range = pick.end > pick.start ? `${pick.start}–${pick.end}` : `${pick.start}`;
    where.textContent = `${pick.file} · ${pick.side} · ${range}`;
    box.focus();
  };

  const drop = () => {
    pick = null;
    anchor = null;
    answering = null;
    box.value = "";
    box.placeholder = asking;
    paint();
  };

  const asking = box.placeholder;

  // Answering parks the composer under the thread it belongs to, the way
  // picking lines parks it under the last line picked.
  const answer = (note) => {
    const thread = document.querySelector(`.thread[data-note="${CSS.escape(note)}"]`);
    if (!thread) return;

    pick = null;
    anchor = null;
    answering = note;

    thread.after(composer);
    composer.hidden = false;
    where.textContent = `answering ${thread.querySelector(".body").textContent.trim().slice(0, 60)}`;
    box.placeholder = "what do you want to say back?";
    box.focus();
  };

  const post = async (path, payload) => {
    const answer = await fetch(`/p/${proposal}/${path}`, {
      method: "POST",
      headers: { "content-type": "application/json" },
      body: JSON.stringify(payload ?? {}),
    });
    if (!answer.ok) alert(await answer.text());
    return answer.ok;
  };

  const leave = async () => {
    if (!box.value.trim()) return;

    if (answering) {
      if (await post("reply", { commentID: answering, body: box.value })) drop();

      return;
    }

    if (!pick) return;

    const ok = await post("comment", {
      selFile: pick.file,
      selSide: pick.side,
      selStart: pick.start,
      selEnd: pick.end,
      body: box.value,
    });

    if (ok) drop();
  };

  // Selecting happens in the gutter, the way it does everywhere else that shows
  // a diff. Dragging down it takes a range, shift extends one, and the code
  // itself stays selectable so you can still copy a line.
  const at = (target) => {
    const gutter = target.closest(".no");
    if (!gutter) return null;

    const line = gutter.closest(".line");
    const no = Number(line?.dataset.no);

    return no ? { file: line.dataset.file, side: line.dataset.side, no } : null;
  };

  let dragging = false;

  document.addEventListener("mousedown", (event) => {
    const spot = at(event.target);
    if (!spot) return;

    event.preventDefault();

    const extend = event.shiftKey && pick && pick.file === spot.file && pick.side === spot.side;

    pick = extend
      ? { ...pick, start: Math.min(anchor ?? pick.start, spot.no), end: Math.max(anchor ?? pick.start, spot.no) }
      : { file: spot.file, side: spot.side, start: spot.no, end: spot.no };

    if (!extend) anchor = spot.no;

    dragging = true;
    paint();
  });

  document.addEventListener("mouseover", (event) => {
    if (!dragging || !pick) return;

    const spot = at(event.target);
    if (!spot || spot.file !== pick.file || spot.side !== pick.side) return;

    pick = { ...pick, start: Math.min(anchor, spot.no), end: Math.max(anchor, spot.no) };
    paint();
  });

  document.addEventListener("mouseup", () => {
    dragging = false;
  });

  // A file folds away when it is not the one being read. The set is kept here
  // rather than in the DOM so a revision arriving does not unfold everything.
  const folded = new Set();

  const fold = () => {
    for (const file of document.querySelectorAll(".file")) {
      file.classList.toggle("folded", folded.has(file.dataset.path));
    }
  };

  // The two columns scroll on their own, so a link between them has to say
  // where it landed. Native anchor jumps put the line at the edge and leave it
  // there unmarked.
  const aim = (id) => {
    const target = document.getElementById(id);
    if (!target) return false;

    target.scrollIntoView({ behavior: "smooth", block: "center" });
    target.classList.remove("aimed");
    void target.offsetWidth;
    target.classList.add("aimed");

    return true;
  };

  document.addEventListener("click", (event) => {
    if (composer.contains(event.target)) return;

    const jump = event.target.closest('a[href^="#"]');
    if (jump && aim(decodeURIComponent(jump.getAttribute("href").slice(1)))) {
      event.preventDefault();

      return;
    }

    if (event.target.closest("[data-fold-all]")) {
      for (const file of document.querySelectorAll(".file")) folded.add(file.dataset.path);
      fold();

      return;
    }

    if (event.target.closest("[data-unfold-all]")) {
      folded.clear();
      fold();

      return;
    }

    const head = event.target.closest(".file > h2");
    if (head) {
      const file = head.closest(".file");
      folded.has(file.dataset.path)
        ? folded.delete(file.dataset.path)
        : folded.add(file.dataset.path);
      fold();

      return;
    }

    const replying = event.target.closest("[data-reply]");
    if (replying) {
      answer(replying.dataset.reply);

      return;
    }

    const resolve = event.target.closest("[data-resolve]");
    if (resolve) post("resolve", { commentID: resolve.dataset.resolve });

    if (event.target.closest("[data-land]")) post("land");

    if (event.target.closest("[data-handover]")) handover();

    if (event.target.closest("[data-abandon]") && confirm("Abandon this proposal?")) {
      post("abandon");
    }
  });

  // The whole review, in one piece, on the clipboard. A reviewer leaves notes
  // for an hour and hands them over once, the way an annotation buffer works.
  const handover = async () => {
    const brief = await (await fetch(`/p/${proposal}/handover`)).text();
    if (!brief.trim()) return;

    // The record is what an agent watching the repository acts on. The
    // clipboard is for the times you are the agent.
    await post("dispatch");

    try {
      await navigator.clipboard.writeText(brief);
      say("Handed over. The notes are on the clipboard too");
    } catch {
      // No clipboard permission: hand it over the way anything else is handed
      // over on a terminal, by leaving it somewhere it can be selected.
      box.value = brief;
      say("Handed over. The clipboard refused, so it is in the box");
    }
  };

  send.addEventListener("click", leave);

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && pick) drop();

    if (event.key === "Enter" && (event.metaKey || event.ctrlKey)) leave();
  });

  // The server watches the repository and pushes the panel whenever what it
  // adds up to changes, so a note the agent answers in your terminal leaves the
  // page without a reload and without taking your selection with it.
  const stream = new EventSource(`/p/${proposal}/events${location.search}`);

  // A stream left open across a navigation keeps its connection, and a browser
  // only gives a host six of them, so the fifth click on the revision strip
  // would starve the page it was asking for. Hang up before leaving.
  window.addEventListener("pagehide", () => stream.close());

  const swap = (id, html) => {
    const node = document.getElementById(id);
    if (node) node.outerHTML = html;
  };

  stream.addEventListener("panel", (event) => {
    swap("panel", event.data);
    fold();
  });

  stream.addEventListener("bar", (event) => swap("bar", event.data));

  // A new revision moves the lines, so the whole page is replaced and the
  // selection goes with it. The composer is parked inside the diff by then:
  // it has to come out before the swap or it is destroyed along with it.
  stream.addEventListener("page", (event) => {
    const page = document.getElementById("page");
    if (!page) return;

    document.body.append(composer);
    page.outerHTML = event.data;
    fold();
    drop();
  });

  paint();
})();
