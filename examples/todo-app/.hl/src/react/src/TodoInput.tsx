import { useState } from "react";
import type { KeyboardEvent } from "react";

type TodoInputProps = {
  onAdd: (text: string) => void;
};

export function TodoInput({ onAdd }: TodoInputProps) {
  const [value, setValue] = useState("");

  function handleKeyDown(event: KeyboardEvent<HTMLInputElement>) {
    if (event.key !== "Enter") {
      return;
    }
    const trimmed = value.trim();
    if (trimmed.length === 0) {
      return;
    }
    onAdd(trimmed);
    setValue("");
  }

  return (
    <input
      className="todo-input"
      type="text"
      placeholder="What needs to be done?"
      value={value}
      onChange={(event) => setValue(event.target.value)}
      onKeyDown={handleKeyDown}
    />
  );
}
