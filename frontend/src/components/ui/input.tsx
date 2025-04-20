import React from "react";
import { cn } from "@/lib/utils";

export interface InputProps
  extends React.InputHTMLAttributes<HTMLInputElement> {
  label?: string;
  error?: string;
  prefixElement?: React.ReactNode;
  suffixElement?: React.ReactNode;
}

/**
 * Input component for Fluxa UI
 * Supports labels, error messages, and prefix/suffix elements
 */
const Input = React.forwardRef<HTMLInputElement, InputProps>(
  (
    {
      className,
      type,
      label,
      error,
      prefixElement,
      suffixElement,
      id,
      ...props
    },
    ref
  ) => {
    // Always call React.useId() unconditionally
    const generatedId = React.useId();
    const inputId = id || generatedId;

    return (
      <div className="w-full">
        {label && (
          <label
            htmlFor={inputId}
            className="mb-2 block text-sm font-medium text-text-dark"
          >
            {label}
          </label>
        )}
        <div className="relative">
          {prefixElement && (
            <div className="absolute inset-y-0 left-0 flex items-center pl-3 pointer-events-none">
              {prefixElement}
            </div>
          )}
          <input
            type={type}
            id={inputId}
            className={cn(
              "flex h-10 w-full rounded-md border border-input bg-background-light px-3 py-2 text-sm placeholder:text-gray-400 focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent disabled:cursor-not-allowed disabled:opacity-50",
              prefixElement && "pl-10",
              suffixElement && "pr-10",
              error && "border-critical focus:ring-critical",
              className
            )}
            ref={ref}
            {...props}
          />
          {suffixElement && (
            <div className="absolute inset-y-0 right-0 flex items-center pr-3 pointer-events-none">
              {suffixElement}
            </div>
          )}
        </div>
        {error && <p className="mt-1 text-xs text-critical">{error}</p>}
      </div>
    );
  }
);

Input.displayName = "Input";

export { Input };
