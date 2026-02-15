// 使用 withGlobalTauri 注入的 window.__TAURI__ 来控制窗口
window.addEventListener("DOMContentLoaded", () => {
  console.debug("[main.js] DOMContentLoaded, 尝试绑定窗口按钮事件");

  const tauriWindowNs = window.__TAURI__?.window;
  console.debug("[main.js] window.__TAURI__ =", window.__TAURI__);
  console.debug(
    "[main.js] window.__TAURI__.window keys =",
    tauriWindowNs ? Object.keys(tauriWindowNs) : null
  );

  // 兼容多种可能的 API 命名：getCurrent / getCurrentWindow / appWindow
  let appWindow = null;
  if (tauriWindowNs) {
    if (typeof tauriWindowNs.getCurrent === "function") {
      appWindow = tauriWindowNs.getCurrent();
      console.debug("[main.js] 使用 window.getCurrent() 获取 appWindow");
    } else if (typeof tauriWindowNs.getCurrentWindow === "function") {
      appWindow = tauriWindowNs.getCurrentWindow();
      console.debug("[main.js] 使用 window.getCurrentWindow() 获取 appWindow");
    } else if (tauriWindowNs.appWindow) {
      appWindow = tauriWindowNs.appWindow;
      console.debug("[main.js] 使用 window.appWindow 获取 appWindow");
    }
  }

  if (!appWindow) {
    console.error(
      "[main.js] 无法获取当前窗口实例，请把上面 window.__TAURI__.window keys 的输出发给助手。"
    );
    return;
  }

  const btnMinimize = document.getElementById("btn-minimize");
  const btnToggleMaximize = document.getElementById("btn-toggle-maximize");
  const btnClose = document.getElementById("btn-close");

  if (btnMinimize) {
    btnMinimize.addEventListener("click", () => {
      console.debug("[main.js] 点击最小化按钮");
      appWindow.minimize();
    });
  } else {
    console.warn("[main.js] 找不到最小化按钮元素");
  }

  if (btnToggleMaximize) {
    btnToggleMaximize.addEventListener("click", () => {
      console.debug("[main.js] 点击最大化/还原按钮");
      appWindow.toggleMaximize();
    });
  } else {
    console.warn("[main.js] 找不到最大化按钮元素");
  }

  if (btnClose) {
    btnClose.addEventListener("click", () => {
      console.debug("[main.js] 点击关闭按钮");
      appWindow.close();
    });
  } else {
    console.warn("[main.js] 找不到关闭按钮元素");
  }
});

