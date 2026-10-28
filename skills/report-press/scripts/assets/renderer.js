(function () {
  var warnings = window.__markdyWarnings = [];
  var dark = matchMedia("(prefers-color-scheme: dark)");
  var still = matchMedia("(prefers-reduced-motion: reduce)");
  var figs = Array.prototype.slice.call(document.querySelectorAll("figure[data-markdy]"));

  // Labels stay legible only near native scale, so each stage is sized from the
  // diagram's own extent and never drawn below MIN_SCALE; wider strips scroll.
  var MIN_SCALE = 0.75, PAD = 56;

  function source(fig) {
    return fig.querySelector('script[type="text/markdy"]').textContent
      .replace("__THEME__", dark.matches ? "midnight" : "editorial");
  }
  function fail(stage, err) {
    stage.innerHTML = '<p class="fallback">Diagram unavailable (' +
      String(err && err.message || err).replace(/[<>&]/g, "") +
      "). The text beside it carries the same content.</p>";
  }
  function size(fig, stage, svg) {
    var m = /viewBox="0 0 ([\d.]+) ([\d.]+)"/.exec(svg);
    if (!m) return;
    var w = +m[1], h = +m[2], avail = fig.querySelector(".scroller").clientWidth;
    var target = Math.max(Math.min(w + PAD, avail), w * MIN_SCALE + PAD);
    // The player resets an inline width, but leaves min/max alone.
    stage.style.minWidth = stage.style.maxWidth = target + "px";
    stage.style.height = Math.round((target - PAD) * h / w + PAD) + "px";
    fig.classList.toggle("wide", target > avail + 1);
  }
  function draw(fig, i) {
    var stage = fig.querySelector(".stage");
    stage.innerHTML = "";
    if (!window.Markdy) return fail(stage, "diagram engine missing");
    var code = source(fig);
    // exportDiagramAsVectorSvg is synchronous in 1.5.7.
    Promise.resolve().then(function () { return Markdy.exportDiagramAsVectorSvg(code); }).then(function (svg) {
      if (typeof svg !== "string" || svg.indexOf("<svg") < 0) throw new Error("no svg produced");
      size(fig, stage, svg);
      if (still.matches) { stage.innerHTML = svg; return; }
      Markdy.createDiagram({
        container: stage, code: code, autoplay: true, loop: true, fit: true, fitMode: "contain",
        onWarning: function (w) { warnings.push("fig " + (i + 1) + " line " + w.line + ": " + w.message); }
      });
    }).catch(function (e) { fail(stage, e); });
  }
  function drawAll() { figs.forEach(draw); }

  figs.forEach(function (fig) {
    var stage = fig.querySelector(".stage"), wrap = document.createElement("div");
    wrap.className = "scroller";
    stage.parentNode.insertBefore(wrap, stage);
    wrap.appendChild(stage);
  });
  drawAll();
  dark.addEventListener("change", drawAll);
  still.addEventListener("change", drawAll);
  var resizeTimer;
  addEventListener("resize", function () { clearTimeout(resizeTimer); resizeTimer = setTimeout(drawAll, 250); });
})();
