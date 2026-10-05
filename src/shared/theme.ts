import { useEffect } from "react";
import { getConfig, onThemeChanged, type ThemeSetting } from "./api";

export function applyTheme(theme: ThemeSetting = "auto") {
  // Auto deliberately leaves resolution to CSS so OS changes apply immediately.
  document.documentElement.dataset.theme = theme;
}

/** Popup windows load the saved preference and stay in sync while open. */
export function useSavedTheme() {
  useEffect(() => {
    let disposed = false;
    let receivedEvent = false;
    const unlisten = onThemeChanged((theme) => {
      receivedEvent = true;
      if (!disposed) applyTheme(theme);
    });
    void unlisten.then(async () => {
      const config = await getConfig();
      if (!disposed && !receivedEvent) applyTheme(config.general.theme);
    }).catch(() => {});
    return () => {
      disposed = true;
      void unlisten.then((fn) => fn()).catch(() => {});
    };
  }, []);
}
