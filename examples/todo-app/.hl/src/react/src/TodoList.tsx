import type { Todo } from "./types";
import { TodoItem } from "./TodoItem";

type TodoListProps = {
  todos: Todo[];
  onToggle: (id: string) => void;
  onDelete: (id: string) => void;
};

export function TodoList({ todos, onToggle, onDelete }: TodoListProps) {
  return (
    <ul className="todo-list">
      {todos.map((todo) => (
        <TodoItem key={todo.id} todo={todo} onToggle={onToggle} onDelete={onDelete} />
      ))}
    </ul>
  );
}
