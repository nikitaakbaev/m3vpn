import { useAppStore } from "../store/useAppStore";

export function Snackbar() {
  const toast = useAppStore((state) => state.toast);
  if (!toast) return null;
  return (
    <div className="m3-snackbar" role="status" aria-live="polite">
      {toast}
    </div>
  );
}
