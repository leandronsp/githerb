// The whole client. It holds which lines are selected and nothing else:
// every other piece of state is rendered by the server and arrives over the
// event stream, which is what keeps the two from disagreeing.
(() => {
  const proposal = document.body.dataset.proposal;
  const composer = document.querySelector(".composer");
  const box = composer?.querySelector("textarea");
  const where = composer?.querySelector(".where");
  const send = composer?.querySelector("button");

  let pick = null;

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
    if (pick) {
      const range = pick.end > pick.start ? `${pick.start}-${pick.end}` : `${pick.start}`;
      where.textContent = `${pick.file} ${pick.side} ${range}`;
      box.focus();
    }
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

  document.addEventListener("click", (event) => {
    const line = event.target.closest(".line");
    if (line) {
      const no = Number(line.dataset.no);
      if (!no) return;

      const extend = event.shiftKey && pick && pick.file === line.dataset.file && pick.side === line.dataset.side;
      pick = extend
        ? { ...pick, start: Math.min(pick.start, no), end: Math.max(pick.start, no) }
        : { file: line.dataset.file, side: line.dataset.side, start: no, end: no };

      paint();
      return;
    }

    const resolve = event.target.closest("[data-resolve]");
    if (resolve) post("resolve", { commentID: resolve.dataset.resolve });

    if (event.target.closest("[data-land]")) post("land");
  });

  send?.addEventListener("click", async () => {
    if (!pick || !box.value.trim()) return;

    const ok = await post("comment", {
      selFile: pick.file,
      selSide: pick.side,
      selStart: pick.start,
      selEnd: pick.end,
      body: box.value,
    });

    if (ok) {
      box.value = "";
      pick = null;
      paint();
    }
  });

  document.addEventListener("keydown", (event) => {
    if (event.key === "Escape" && pick) {
      pick = null;
      paint();
    }
  });

  // The server watches the repository and pushes the panel whenever what it
  // adds up to changes, so an annotation an agent answers in the terminal lands
  // here without a reload.
  const stream = new EventSource(`/p/${proposal}/events`);
  stream.addEventListener("panel", (event) => {
    const panel = document.getElementById("panel");
    if (panel) panel.outerHTML = event.data;
  });

  paint();
})();
