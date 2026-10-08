import { createContext, useContext } from "react";
export interface PlatformCapabilities { os: string; ble: boolean; caret: boolean; shortcut: boolean }
export const PlatformContext = createContext<PlatformCapabilities>({ os: "unknown", ble: false, caret: false, shortcut: false });
export const usePlatform = () => useContext(PlatformContext);
