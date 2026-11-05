import type { ButtonHTMLAttributes } from "react";

type Variant = "primary" | "ghost";

const VARIANTS: Record<Variant, string> = {
  primary: "bg-accent text-accent-ink hover:brightness-110",
  ghost: "border border-line text-ink hover:bg-raised",
};

interface ButtonProps extends ButtonHTMLAttributes<HTMLButtonElement> {
  variant?: Variant;
}

/** A button that is always `type="button"` unless told otherwise, so it never submits a form by accident. */
export function Button({ variant = "ghost", type = "button", className = "", ...rest }: ButtonProps) {
  return (
    <button
      type={type}
      className={`ui-transition inline-flex items-center gap-2 rounded-md px-3 py-1.5 text-sm font-medium disabled:cursor-not-allowed disabled:opacity-50 ${VARIANTS[variant]} ${className}`}
      {...rest}
    />
  );
}
