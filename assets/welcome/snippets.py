# A tiny scratchpad for testing syntax highlighting and ghost completions.
# Save the file, then open the preview panel (Ctrl+Alt+P).

from dataclasses import dataclass


@dataclass
class Task:
    title: str
    done: bool = False

    def complete(self) -> "Task":
        self.done = True
        return self


def summarize(tasks: list[Task]) -> str:
    finished = sum(1 for task in tasks if task.done)
    return f"{finished}/{len(tasks)} tasks completed"


if __name__ == "__main__":
    tasks = [Task("Open Kodemo"), Task("Write a little Rust"), Task("Ship it")]
    tasks[0].complete()
    print(summarize(tasks))
