import { useMemo, useState } from "react";
import type { Filter, Todo } from "./types";
import { TodoInput } from "./TodoInput";
import { TodoList } from "./TodoList";
import { FilterControl } from "./FilterControl";

let nextId = 1;

function createTodo(text: string): Todo {
  const id = String(nextId);
  nextId += 1;
  return { id, text, done: false };
}

function filterTodos(todos: Todo[], filter: Filter): Todo[] {
  if (filter === "active") {
    return todos.filter((todo) => !todo.done);
  }
  if (filter === "completed") {
    return todos.filter((todo) => todo.done);
  }
  return todos;
}

export function App() {
  const [todos, setTodos] = useState<Todo[]>([]);
  const [filter, setFilter] = useState<Filter>("all");

  const visibleTodos = useMemo(() => filterTodos(todos, filter), [todos, filter]);
  const remainingCount = useMemo(() => todos.filter((todo) => !todo.done).length, [todos]);

  function handleAdd(text: string) {
    setTodos((prev) => [...prev, createTodo(text)]);
  }

  function handleToggle(id: string) {
    setTodos((prev) =>
      prev.map((todo) => (todo.id === id ? { ...todo, done: !todo.done } : todo)),
    );
  }

  function handleDelete(id: string) {
    setTodos((prev) => prev.filter((todo) => todo.id !== id));
  }

  return (
    <div className="app">
      <h1>Todo App</h1>
      <TodoInput onAdd={handleAdd} />
      <FilterControl filter={filter} onFilterChange={setFilter} />
      <TodoList todos={visibleTodos} onToggle={handleToggle} onDelete={handleDelete} />
      <p className="remaining-count">{remainingCount} items left</p>
    </div>
  );
}
