import React, { createContext, useCallback, useContext, useState } from "react";

export function cx(...parts: Array<string | false | null | undefined>): string {
  return parts.filter(Boolean).join(" ");
}

// ─── Buttons ──────────────────────────────────────────────

type BtnVariant = "primary" | "ghost" | "danger" | "subtle";
const BTN: Record<BtnVariant, string> = {
  primary: "bg-indigo-600 hover:bg-indigo-500 text-white border-indigo-500",
  danger: "bg-red-600/90 hover:bg-red-500 text-white border-red-500",
  ghost: "bg-transparent hover:bg-white/5 text-zinc-200 border-white/10",
  subtle: "bg-white/5 hover:bg-white/10 text-zinc-200 border-white/10",
};

export function Button({
  variant = "subtle",
  className,
  ...props
}: React.ButtonHTMLAttributes<HTMLButtonElement> & { variant?: BtnVariant }) {
  return (
    <button
      {...props}
      className={cx(
        "inline-flex items-center gap-1.5 rounded-md border px-3 py-1.5 text-sm font-medium",
        "transition-colors disabled:opacity-40 disabled:cursor-not-allowed",
        BTN[variant],
        className,
      )}
    />
  );
}

// ─── Cards / layout ───────────────────────────────────────

export function Card({ className, children }: { className?: string; children: React.ReactNode }) {
  return (
    <div className={cx("rounded-xl border border-white/10 bg-zinc-900/50 p-4", className)}>
      {children}
    </div>
  );
}

export function SectionTitle({ children, hint }: { children: React.ReactNode; hint?: string }) {
  return (
    <div className="mb-3 flex items-baseline justify-between">
      <h2 className="text-lg font-semibold text-zinc-100">{children}</h2>
      {hint && <span className="text-xs text-zinc-500">{hint}</span>}
    </div>
  );
}

// ─── Form controls ────────────────────────────────────────

export function Label({ children }: { children: React.ReactNode }) {
  return <label className="mb-1 block text-xs font-medium text-zinc-400">{children}</label>;
}

const CTRL =
  "w-full rounded-md border border-white/10 bg-zinc-950/60 px-3 py-1.5 text-sm text-zinc-100 " +
  "outline-none focus:border-indigo-500/70 placeholder:text-zinc-600";

export function Input(props: React.InputHTMLAttributes<HTMLInputElement>) {
  return <input {...props} className={cx(CTRL, props.className)} />;
}
export function Textarea(props: React.TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea {...props} className={cx(CTRL, "font-mono", props.className)} />;
}
export function Select(props: React.SelectHTMLAttributes<HTMLSelectElement>) {
  return <select {...props} className={cx(CTRL, props.className)} />;
}

export function Field({
  label,
  children,
}: {
  label: React.ReactNode;
  children: React.ReactNode;
}) {
  return (
    <div className="mb-3">
      <Label>{label}</Label>
      {children}
    </div>
  );
}

export function Badge({
  children,
  tone = "zinc",
}: {
  children: React.ReactNode;
  tone?: "zinc" | "green" | "red" | "indigo" | "amber";
}) {
  const tones: Record<string, string> = {
    zinc: "bg-white/5 text-zinc-300 border-white/10",
    green: "bg-emerald-500/15 text-emerald-300 border-emerald-500/30",
    red: "bg-red-500/15 text-red-300 border-red-500/30",
    indigo: "bg-indigo-500/15 text-indigo-300 border-indigo-500/30",
    amber: "bg-amber-500/15 text-amber-300 border-amber-500/30",
  };
  return (
    <span
      className={cx(
        "inline-flex items-center rounded-full border px-2 py-0.5 text-[11px] font-medium",
        tones[tone],
      )}
    >
      {children}
    </span>
  );
}

// ─── Modal ────────────────────────────────────────────────

export function Modal({
  title,
  onClose,
  children,
  wide,
}: {
  title: React.ReactNode;
  onClose: () => void;
  children: React.ReactNode;
  wide?: boolean;
}) {
  return (
    <div
      className="fixed inset-0 z-50 flex items-start justify-center overflow-y-auto bg-black/60 p-4 py-10"
      onMouseDown={onClose}
    >
      <div
        className={cx(
          "w-full rounded-2xl border border-white/10 bg-zinc-900 p-5 shadow-2xl",
          wide ? "max-w-3xl" : "max-w-xl",
        )}
        onMouseDown={(e) => e.stopPropagation()}
      >
        <div className="mb-4 flex items-center justify-between">
          <h3 className="text-base font-semibold text-zinc-100">{title}</h3>
          <button onClick={onClose} className="text-zinc-500 hover:text-zinc-200" aria-label="Close">
            ✕
          </button>
        </div>
        {children}
      </div>
    </div>
  );
}

// ─── Toasts ───────────────────────────────────────────────

type Toast = { id: number; kind: "ok" | "err"; msg: string };
const ToastCtx = createContext<(kind: "ok" | "err", msg: string) => void>(() => {});
export const useToast = () => useContext(ToastCtx);

let toastSeq = 1;

export function ToastProvider({ children }: { children: React.ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const push = useCallback((kind: "ok" | "err", msg: string) => {
    const id = toastSeq++;
    setToasts((t) => [...t, { id, kind, msg }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), 4200);
  }, []);
  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div className="fixed bottom-4 right-4 z-[60] flex w-80 flex-col gap-2">
        {toasts.map((t) => (
          <div
            key={t.id}
            className={cx(
              "rounded-lg border px-3 py-2 text-sm shadow-lg",
              t.kind === "ok"
                ? "border-emerald-500/40 bg-emerald-950/80 text-emerald-200"
                : "border-red-500/40 bg-red-950/80 text-red-200",
            )}
          >
            {t.msg}
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}

/** Wrap an async action with toast feedback + optional refresh. */
export function useAction() {
  const toast = useToast();
  return useCallback(
    async (fn: () => Promise<unknown>, okMsg?: string, after?: () => void) => {
      try {
        await fn();
        if (okMsg) toast("ok", okMsg);
        after?.();
      } catch (e) {
        toast("err", e instanceof Error ? e.message : String(e));
      }
    },
    [toast],
  );
}
