import { describe, expect, it } from "vitest";
import { render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { App } from "./App";

function getInput() {
  return screen.getByPlaceholderText("What needs to be done?") as HTMLInputElement;
}

async function addTodo(user: ReturnType<typeof userEvent.setup>, text: string) {
  const input = getInput();
  await user.type(input, `${text}{Enter}`);
}

describe("App", () => {
  it("adds a todo on Enter, trims whitespace, and clears the input", async () => {
    const user = userEvent.setup();
    render(<App />);

    await addTodo(user, "  Buy milk  ");

    expect(screen.getByText("Buy milk")).toBeInTheDocument();
    expect(getInput().value).toBe("");
  });

  it("ignores empty or whitespace-only input", async () => {
    const user = userEvent.setup();
    render(<App />);

    await user.type(getInput(), "   {Enter}");

    expect(screen.queryByRole("listitem")).not.toBeInTheDocument();
  });

  it("toggles a todo's done state and shows strikethrough", async () => {
    const user = userEvent.setup();
    render(<App />);

    await addTodo(user, "Walk the dog");
    const checkbox = screen.getByRole("checkbox", { name: "Toggle Walk the dog" });
    const text = screen.getByText("Walk the dog");

    expect(text).not.toHaveClass("done");

    await user.click(checkbox);

    expect(text).toHaveClass("done");
    expect(checkbox).toBeChecked();

    await user.click(checkbox);

    expect(text).not.toHaveClass("done");
  });

  it("deletes a todo", async () => {
    const user = userEvent.setup();
    render(<App />);

    await addTodo(user, "Read a book");
    expect(screen.getByText("Read a book")).toBeInTheDocument();

    await user.click(screen.getByRole("button", { name: "Delete Read a book" }));

    expect(screen.queryByText("Read a book")).not.toBeInTheDocument();
  });

  it("filters todos between All, Active, and Completed and indicates the active filter", async () => {
    const user = userEvent.setup();
    render(<App />);

    await addTodo(user, "Active task");
    await addTodo(user, "Done task");
    await user.click(screen.getByRole("checkbox", { name: "Toggle Done task" }));

    const allButton = screen.getByRole("button", { name: "All" });
    const activeButton = screen.getByRole("button", { name: "Active" });
    const completedButton = screen.getByRole("button", { name: "Completed" });

    expect(allButton).toHaveClass("active");

    await user.click(activeButton);
    expect(activeButton).toHaveClass("active");
    expect(allButton).not.toHaveClass("active");
    expect(screen.getByText("Active task")).toBeInTheDocument();
    expect(screen.queryByText("Done task")).not.toBeInTheDocument();

    await user.click(completedButton);
    expect(completedButton).toHaveClass("active");
    expect(screen.queryByText("Active task")).not.toBeInTheDocument();
    expect(screen.getByText("Done task")).toBeInTheDocument();

    await user.click(allButton);
    expect(screen.getByText("Active task")).toBeInTheDocument();
    expect(screen.getByText("Done task")).toBeInTheDocument();
  });

  it("shows a remaining-items counter for active todos", async () => {
    const user = userEvent.setup();
    render(<App />);

    expect(screen.getByText("0 items left")).toBeInTheDocument();

    await addTodo(user, "Task one");
    await addTodo(user, "Task two");
    expect(screen.getByText("2 items left")).toBeInTheDocument();

    await user.click(screen.getByRole("checkbox", { name: "Toggle Task one" }));
    expect(screen.getByText("1 items left")).toBeInTheDocument();
  });
});
