(() => {
  if (window.__velocityCompanionLoaded) return;
  window.__velocityCompanionLoaded = true;

  const blockedHosts = [
    "netflix.com",
    "disneyplus.com",
    "primevideo.com",
    "hulu.com",
    "max.com"
  ];
  if (blockedHosts.some((host) => location.hostname === host || location.hostname.endsWith(`.${host}`))) return;

  let host;
  let button;
  let dismissedUrl = "";
  let currentUrl = location.href;

  function velocityUrl() {
    const source = encodeURIComponent(location.href);
    const title = encodeURIComponent(document.title.slice(0, 180));
    return `velocity://download?url=${source}&type=media&title=${title}&mode=video&resolution=best`;
  }

  function launchVelocity() {
    const anchor = document.createElement("a");
    anchor.href = velocityUrl();
    anchor.style.display = "none";
    (document.body || document.documentElement).appendChild(anchor);
    anchor.click();
    anchor.remove();
    button.textContent = "Sent to Velocity";
    setTimeout(() => { if (button) button.textContent = "Download this video"; }, 1800);
  }

  function createOverlay() {
    host = document.createElement("div");
    host.id = "velocity-download-companion";
    host.style.cssText = "all:initial;position:fixed;z-index:2147483647;display:none;pointer-events:auto";
    const shadow = host.attachShadow({ mode: "closed" });
    const style = document.createElement("style");
    style.textContent = `
      .wrap{display:flex;align-items:stretch;filter:drop-shadow(0 8px 22px rgba(0,0,0,.42));font:600 13px/1.2 Inter,Segoe UI,Arial,sans-serif}
      button{border:0;cursor:pointer;color:#fff;background:#6955ee}
      .download{height:38px;padding:0 15px;border-radius:7px 0 0 7px;display:flex;gap:9px;align-items:center;white-space:nowrap}
      .download:hover{background:#7865f4}.download:focus-visible,.close:focus-visible{outline:2px solid #fff;outline-offset:2px}
      .mark{width:17px;height:17px;border:2px solid currentColor;border-radius:50%;display:grid;place-items:center;font-size:12px;line-height:1}
      .close{width:30px;border-left:1px solid rgba(255,255,255,.22);border-radius:0 7px 7px 0;background:#5846cf;font-size:17px;font-weight:400}
      .close:hover{background:#4f3fc0}
    `;
    const wrap = document.createElement("div");
    wrap.className = "wrap";
    button = document.createElement("button");
    button.className = "download";
    button.type = "button";
    button.setAttribute("aria-label", "Download this video with Velocity");
    button.innerHTML = '<span class="mark">↓</span><span>Download this video</span>';
    button.addEventListener("click", launchVelocity);
    const close = document.createElement("button");
    close.className = "close";
    close.type = "button";
    close.setAttribute("aria-label", "Hide Velocity button on this page");
    close.textContent = "×";
    close.addEventListener("click", () => {
      dismissedUrl = location.href;
      host.style.display = "none";
    });
    wrap.append(button, close);
    shadow.append(style, wrap);
    (document.fullscreenElement || document.documentElement).appendChild(host);
  }

  function largestVisibleVideo() {
    let best = null;
    let bestArea = 0;
    for (const video of document.querySelectorAll("video")) {
      const rect = video.getBoundingClientRect();
      const style = getComputedStyle(video);
      const area = Math.max(0, rect.width) * Math.max(0, rect.height);
      if (style.display !== "none" && style.visibility !== "hidden" && rect.bottom > 0 && rect.top < innerHeight && area > 70000 && area > bestArea) {
        best = video;
        bestArea = area;
      }
    }
    return best;
  }

  function updateOverlay() {
    if (location.href !== currentUrl) {
      currentUrl = location.href;
      dismissedUrl = "";
    }
    const fullscreenParent = document.fullscreenElement || document.documentElement;
    if (host && host.parentNode !== fullscreenParent) fullscreenParent.appendChild(host);
    const video = largestVisibleVideo();
    if (!video || dismissedUrl === location.href) {
      if (host) host.style.display = "none";
      return;
    }
    if (!host) createOverlay();
    const rect = video.getBoundingClientRect();
    const width = 185;
    host.style.left = `${Math.max(10, Math.min(innerWidth - width - 10, rect.right - width - 12))}px`;
    host.style.top = `${Math.max(10, rect.top + 12)}px`;
    host.style.display = "block";
  }

  const observer = new MutationObserver(updateOverlay);
  observer.observe(document.documentElement, { childList: true, subtree: true });
  addEventListener("resize", updateOverlay, { passive: true });
  addEventListener("scroll", updateOverlay, { passive: true });
  addEventListener("fullscreenchange", updateOverlay);
  setInterval(updateOverlay, 1000);
  updateOverlay();
})();
