'use client';

import React, { useState, useEffect, useRef } from "react";
import { cn } from "@/lib/utils";

export interface SelectOption {
  value: string;
  label: string;
  icon?: React.ReactNode;
}

export interface SelectProps {
  options: SelectOption[];
  value?: string;
  onChange?: (value: string) => void;
  placeholder?: string;
  label?: string;
  disabled?: boolean;
  error?: string;
  className?: string;
}

/**
 * Custom Select component for Fluxa UI
 * Supports icons, custom styling, and full keyboard navigation
 */
export const Select = ({
  options,
  value,
  onChange,
  placeholder = "Select an option",
  label,
  disabled,
  error,
  className,
}: SelectProps) => {
  const [isOpen, setIsOpen] = useState(false);
  const [selectedValue, setSelectedValue] = useState(value || "");
  const selectRef = useRef<HTMLDivElement>(null);

  // Close dropdown when clicking outside
  useEffect(() => {
    const handleClickOutside = (event: MouseEvent) => {
      if (selectRef.current && !selectRef.current.contains(event.target as Node)) {
        setIsOpen(false);
      }
    };

    document.addEventListener("mousedown", handleClickOutside);
    return () => {
      document.removeEventListener("mousedown", handleClickOutside);
    };
  }, []);

  // Update internal state when value prop changes
  useEffect(() => {
    if (value !== undefined) {
      setSelectedValue(value);
    }
  }, [value]);

  const handleSelect = (option: SelectOption) => {
    setSelectedValue(option.value);
    onChange?.(option.value);
    setIsOpen(false);
  };

  const selectedOption = options.find(option => option.value === selectedValue);
  
  return (
    <div className="relative w-full">
      {label && (
        <label className="mb-2 block text-sm font-medium text-text-dark">
          {label}
        </label>
      )}
      <div
        ref={selectRef}
        className={cn(
          "relative w-full cursor-pointer",
          disabled && "opacity-60 cursor-not-allowed",
          className
        )}
      >
        <div
          className={cn(
            "flex h-10 w-full items-center justify-between rounded-md border border-input bg-background-light px-3 py-2 text-sm",
            "focus:outline-none focus:ring-2 focus:ring-primary focus:border-transparent",
            error && "border-critical focus:ring-critical",
            isOpen && "ring-2 ring-primary border-transparent",
            disabled && "bg-gray-100 cursor-not-allowed"
          )}
          onClick={() => !disabled && setIsOpen(!isOpen)}
          tabIndex={0}
          role="combobox"
          aria-expanded={isOpen}
          aria-controls="select-dropdown"
        >
          <div className="flex items-center">
            {selectedOption?.icon && (
              <span className="mr-2">{selectedOption.icon}</span>
            )}
            <span className={!selectedValue ? "text-gray-400" : ""}>
              {selectedOption ? selectedOption.label : placeholder}
            </span>
          </div>
          <svg
            xmlns="http://www.w3.org/2000/svg"
            width="16"
            height="16"
            viewBox="0 0 24 24"
            fill="none"
            stroke="currentColor"
            strokeWidth="2"
            strokeLinecap="round"
            strokeLinejoin="round"
            className={cn(
              "transition-transform",
              isOpen ? "transform rotate-180" : ""
            )}
          >
            <path d="m6 9 6 6 6-6" />
          </svg>
        </div>

        {isOpen && (
          <ul
            id="select-dropdown"
            className="absolute z-10 mt-1 max-h-60 w-full overflow-auto rounded-md border border-gray-200 bg-background-light py-1 shadow-lg"
          >
            {options.map((option) => (
              <li
                key={option.value}
                className={cn(
                  "flex items-center px-3 py-2 text-sm hover:bg-primary hover:text-white cursor-pointer",
                  option.value === selectedValue && "bg-primary/10 font-medium"
                )}
                onClick={() => handleSelect(option)}
              >
                {option.icon && <span className="mr-2">{option.icon}</span>}
                {option.label}
              </li>
            ))}
          </ul>
        )}
      </div>
      {error && <p className="mt-1 text-xs text-critical">{error}</p>}
    </div>
  );
};

export default Select;