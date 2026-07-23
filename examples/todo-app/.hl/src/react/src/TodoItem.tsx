import type { Todo } from "./types";

type TodoItemProps = {
  todo: Todo;
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
};

export function TodoItem({ todo, onToggle, onDelete }: TodoItemProps) {
  return (
    <li className="todo-item">
      <input
        type="checkbox"
        checked={todo.done}
        onChange={() => onToggle(todo.id)}
        aria-label={`Toggle ${todo.text}`}
      />
      <span className={todo.done ? "todo-text done" : "todo-text"}>{todo.text}</span>
      <button type="button" onClick={() => onDelete(todo.id)} aria-label={`Delete ${todo.text}`}>
        Delete
      </button>
    </li>
  );
}
