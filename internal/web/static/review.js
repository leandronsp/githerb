// The whole client. It knows which lines are picked and nothing else: every
// other piece of state is rendered by the server and arrives over the event
// stream, which is what keeps the two from ever disagreeing.
(() => {
  const proposal = document.body.dataset.proposal;
  const composer = document.querySelector(".composer");
  const box = composer.querySelector("textarea");
  const where = composer.querySelector(".where");
  const send = composer.querySelector("button");

  let pick = null;
  let anchor = null;

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

    composer.hidden = !pick;
    if (!pick) return;

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
    box.value = "";
    paint();
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
    if (!pick || !box.value.trim()) return;

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

  document.addEventListener("click", (event) => {
    if (composer.contains(event.target)) return;

    const resolve = event.target.closest("[data-resolve]");
    if (resolve) post("resolve", { commentID: resolve.dataset.resolve });

    if (event.target.closest("[data-land]")) post("land");

    const hand = event.target.closest("[data-handover]");
    if (hand) handover(hand);

    if (event.target.closest("[data-abandon]") && confirm("Give up on this proposal?")) {
      post("abandon");
    }
  });

  // The whole review, in one piece, on the clipboard. A reviewer leaves notes
  // for an hour and hands them over once, the way an annotation buffer works.
  const handover = async (button) => {
    const brief = await (await fetch(`/p/${proposal}/handover`)).text();
    if (!brief.trim()) return;

    const said = button.textContent;

    try {
      await navigator.clipboard.writeText(brief);
      button.textContent = "Copied. Paste it to your agent";
    } catch {
      // No clipboard permission: hand it over the way anything else is handed
      // over on a terminal, by leaving it somewhere it can be selected.
      box.value = brief;
      button.textContent = "Clipboard refused. It is in the box";
    }

    button.dataset.handedOver = "1";
    setTimeout(() => (button.textContent = said), 4000);
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

  stream.addEventListener("panel", (event) => {
    const panel = document.getElementById("panel");
    if (panel) panel.outerHTML = event.data;
  });

  // A new revision moves the lines, so the whole page is replaced and the
  // selection goes with it. The composer is parked inside the diff by then:
  // it has to come out before the swap or it is destroyed along with it.
  stream.addEventListener("page", (event) => {
    const page = document.getElementById("page");
    if (!page) return;

    document.body.append(composer);
    page.outerHTML = event.data;
    drop();
  });

  paint();
})();
