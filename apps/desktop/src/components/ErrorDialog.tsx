import { useEffect, useRef } from "react";
import { Button } from "./Button";
import { Icon } from "./Icon";
import { useAppStore, useT } from "../store/useAppStore";

export function ErrorDialog() {
  const t = useT();
  const error = useAppStore((state) => state.lastError);
  const clearError = useAppStore((state) => state.clearError);
  const openCoreDir = useAppStore((state) => state.openCoreDir);
  const dialogRef = useRef<HTMLDialogElement>(null);

  useEffect(() => {
    const dialog = dialogRef.current;
    if (!dialog) return;
    if (error && !dialog.open) {
      dialog.showModal();
    } else if (!error && dialog.open) {
      dialog.close();
    }
  }, [error]);

  return (
    <dialog ref={dialogRef} className="m3-dialog" aria-labelledby="error-title" onClose={clearError}>
      <div className="m3-dialog__head">
        <span className="m3-dialog__icon">
          <Icon name="error" size={22} filled />
        </span>
        <div>
          <h2 id="error-title">{t("error.title")}</h2>
          <p>{t("error.hint")}</p>
        </div>
      </div>
      <pre className="m3-dialog__body">{error}</pre>
      <div className="m3-dialog__actions">
        <Button variant="text" onClick={clearError}>
          {t("error.close")}
        </Button>
        <Button variant="tonal" icon="folder_open" onClick={() => void openCoreDir()}>
          {t("error.logs")}
        </Button>
      </div>
    </dialog>
  );
}
