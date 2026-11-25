//! Starter code written into newly created files.
//!
//! A brand-new empty file is not much help to a beginner, so Kodemo fills it
//! with a tiny, runnable example that matches the language of its extension.
//! The samples are deliberately short: they teach the shape of a file without
//! burying the user in boilerplate.

use crate::syntax::Language;
use std::path::Path;

pub fn sample_for_path(path: &Path) -> Option<&'static str> {
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    let ext = path
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if name == "dockerfile" {
        return Some(DOCKERFILE);
    }
    if name == "makefile" {
        return Some(MAKEFILE);
    }
    let sample = match ext.as_str() {
        "html" | "htm" | "xhtml" => HTML,
        "css" | "scss" | "sass" | "less" => CSS,
        "js" | "mjs" | "cjs" => JAVASCRIPT,
        "jsx" => JSX,
        "ts" => TYPESCRIPT,
        "tsx" => TSX,
        "vue" => VUE,
        "py" | "pyw" => PYTHON,
        "rs" => RUST,
        "c" | "h" => C,
        "cpp" | "cc" | "cxx" | "hpp" | "hh" => CPP,
        "cs" => CSHARP,
        "java" => JAVA,
        "kt" | "kts" => KOTLIN,
        "go" => GO,
        "rb" => RUBY,
        "php" => PHP,
        "lua" => LUA,
        "swift" => SWIFT,
        "dart" => DART,
        "sql" => SQL,
        "sh" | "bash" | "zsh" => SHELL,
        "json" | "jsonc" | "json5" => JSON,
        "md" | "markdown" => MARKDOWN,
        "toml" => TOML,
        "yaml" | "yml" => YAML,
        "xml" | "svg" => XML,
        "ini" | "conf" | "cfg" => INI,
        _ => {
            return match Language::from_path(path) {
                Language::Plain => None,
                _ => Some(PLAIN),
            }
        }
    };
    Some(sample)
}

const HTML: &str = r#"<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8" />
  <meta name="viewport" content="width=device-width, initial-scale=1.0" />
  <title>My page</title>
  <link rel="stylesheet" href="styles.css" />
</head>
<body>
  <h1>Hello, world!</h1>
  <p>Start editing this file in Kodemo.</p>
  <script src="app.js"></script>
</body>
</html>
"#;

const CSS: &str = r#":root {
  --bg: #1e1e1e;
  --fg: #f2f2f2;
  --accent: #7aa2f7;
}

body {
  margin: 0;
  font-family: system-ui, sans-serif;
  background: var(--bg);
  color: var(--fg);
}

h1 {
  color: var(--accent);
}
"#;

const JAVASCRIPT: &str = r#"// A small starting point — change it as you like.
const message = "Hello, world!";

function greet(name) {
  return `Hello, ${name}!`;
}

console.log(greet("Kodemo"));
console.log(message);
"#;

const TYPESCRIPT: &str = r#"interface User {
  name: string;
  age: number;
}

function greet(user: User): string {
  return `Hello, ${user.name}!`;
}

const user: User = { name: "Kodemo", age: 1 };
console.log(greet(user));
"#;

const JSX: &str = r#"export default function App() {
  return (
    <main>
      <h1>Hello, world!</h1>
      <p>Start editing this component in Kodemo.</p>
    </main>
  );
}
"#;

const TSX: &str = r#"type Props = {
  name: string;
};

export default function App({ name }: Props) {
  return (
    <main>
      <h1>Hello, {name}!</h1>
      <p>Start editing this component in Kodemo.</p>
    </main>
  );
}
"#;

const VUE: &str = r#"<script setup>
import { ref } from "vue";

const message = ref("Hello, world!");
</script>

<template>
  <main>
    <h1>{{ message }}</h1>
  </main>
</template>

<style scoped>
h1 {
  font-family: system-ui, sans-serif;
}
</style>
"#;

const PYTHON: &str = r#""""A small starting point — change it as you like."""


def greet(name: str) -> str:
    return f"Hello, {name}!"


def main() -> None:
    print(greet("Kodemo"))


if __name__ == "__main__":
    main()
"#;

const RUST: &str = r#"fn greet(name: &str) -> String {
    format!("Hello, {name}!")
}

fn main() {
    println!("{}", greet("Kodemo"));
}
"#;

const C: &str = r#"#include <stdio.h>

int main(void) {
    printf("Hello, world!\n");
    return 0;
}
"#;

const CPP: &str = r#"#include <iostream>
#include <string>

std::string greet(const std::string &name) {
    return "Hello, " + name + "!";
}

int main() {
    std::cout << greet("Kodemo") << std::endl;
    return 0;
}
"#;

const CSHARP: &str = r#"using System;

class Program
{
    static string Greet(string name) => $"Hello, {name}!";

    static void Main()
    {
        Console.WriteLine(Greet("Kodemo"));
    }
}
"#;

const JAVA: &str = r#"public class Main {
    static String greet(String name) {
        return "Hello, " + name + "!";
    }

    public static void main(String[] args) {
        System.out.println(greet("Kodemo"));
    }
}
"#;

const KOTLIN: &str = r#"fun greet(name: String): String = "Hello, $name!"

fun main() {
    println(greet("Kodemo"))
}
"#;

const GO: &str = r#"package main

import "fmt"

func greet(name string) string {
	return "Hello, " + name + "!"
}

func main() {
	fmt.Println(greet("Kodemo"))
}
"#;

const RUBY: &str = r#"def greet(name)
  "Hello, #{name}!"
end

puts greet("Kodemo")
"#;

const PHP: &str = r#"<?php

function greet(string $name): string {
    return "Hello, $name!";
}

echo greet("Kodemo") . PHP_EOL;
"#;

const LUA: &str = r#"local function greet(name)
  return "Hello, " .. name .. "!"
end

print(greet("Kodemo"))
"#;

const SWIFT: &str = r#"func greet(_ name: String) -> String {
    return "Hello, \(name)!"
}

print(greet("Kodemo"))
"#;

const DART: &str = r#"String greet(String name) => 'Hello, $name!';

void main() {
  print(greet('Kodemo'));
}
"#;

const SQL: &str = r#"-- A small starting point — change it as you like.
CREATE TABLE greetings (
    id INTEGER PRIMARY KEY,
    message TEXT NOT NULL
);

INSERT INTO greetings (message) VALUES ('Hello, world!');

SELECT * FROM greetings;
"#;

const SHELL: &str = r#"#!/usr/bin/env bash
set -euo pipefail

name="${1:-Kodemo}"
echo "Hello, ${name}!"
"#;

const JSON: &str = r#"{
  "name": "my-project",
  "version": "0.1.0",
  "private": true,
  "scripts": {
    "start": "echo \"Hello, world!\""
  }
}
"#;

const MARKDOWN: &str = r#"# My project

A short description of what this project does.

## Getting started

1. Open the folder in Kodemo.
2. Edit the files.
3. Run the project with **Ctrl+F5**.

## License

MIT
"#;

const TOML: &str = r#"[package]
name = "my-project"
version = "0.1.0"
edition = "2021"

[dependencies]
"#;

const YAML: &str = r#"name: my-project
version: 0.1.0
description: A small starting point

steps:
  - name: greet
    run: echo "Hello, world!"
"#;

const XML: &str = r#"<?xml version="1.0" encoding="UTF-8"?>
<root>
  <message>Hello, world!</message>
</root>
"#;

const INI: &str = r#"[settings]
name = my-project
version = 0.1.0
enabled = true
"#;

const DOCKERFILE: &str = r#"FROM debian:13-slim

WORKDIR /app
COPY . .
CMD ["echo", "Hello, world!"]
"#;

const MAKEFILE: &str = r#"all:
	@echo "Hello, world!"
"#;

const PLAIN: &str = r#"Hello, world!

Start editing this file in Kodemo.
"#;

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn samples_match_the_extension() {
        assert!(sample_for_path(&PathBuf::from("index.html")).unwrap().contains("<!DOCTYPE html>"));
        assert!(sample_for_path(&PathBuf::from("main.py")).unwrap().contains("def greet"));
        assert!(sample_for_path(&PathBuf::from("lib.rs")).unwrap().contains("fn main"));
        assert!(sample_for_path(&PathBuf::from("app.tsx")).unwrap().contains("function App"));
        assert!(sample_for_path(&PathBuf::from("Makefile")).unwrap().contains("all:"));
        assert!(sample_for_path(&PathBuf::from("notes.unknownext")).is_none());
    }
}
