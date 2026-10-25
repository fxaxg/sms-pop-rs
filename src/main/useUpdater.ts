import { useEffect, useMemo, useState, useSyncExternalStore } from "react";
import { checkUpdate, downloadUpdate, getUpdateStatus, installUpdate, onUpdateStatus } from "../shared/api";
import { UpdateController } from "../shared/updater";
import type { PreviewUpdateBackend } from "./dev/previewUpdater";

const backend = {
  listen: onUpdateStatus, query: getUpdateStatus, check: checkUpdate,
  download: downloadUpdate, install: installUpdate,
};

export function useUpdater() {
  const [preview, setPreview] = useState<PreviewUpdateBackend | null>(null);
  const controller = useMemo(() => new UpdateController(preview ?? backend), [preview]);
  useEffect(() => controller.connect(), [controller]);
  useEffect(() => () => preview?.dispose(), [preview]);
  const startPreview = import.meta.env.DEV ? async () => {
    const { PreviewUpdateBackend } = await import("./dev/previewUpdater");
    setPreview((current) => current ?? new PreviewUpdateBackend());
  } : undefined;
  return {
    ...useSyncExternalStore(controller.subscribe, controller.snapshot), controller,
    preview, startPreview, stopPreview: () => setPreview(null),
  };
}

export type Updater = ReturnType<typeof useUpdater>;
