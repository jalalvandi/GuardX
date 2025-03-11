# GuardX
**GuardX** is a simple command-line (CLI) tool for managing folders and files with encryption capabilities. Built with **Rust** and the **ratatui** library, it offers a visually appealing terminal-based user interface. Its goal is to keep your files secure while providing a delightful user experience!

---

## Features
- 📁 **Folder & File Management:** Browse, create, and delete folders and files (Not Complete Yet)
- 🔐 **Encryption & Decryption:** Protect folders with a custom encryption key.
- 📄 **File Preview:** View file contents directly in the app.
- ⚙ **Custom Settings:** Switch between dark/light themes and adjust key length.
- 📊 **History & Dashboard:** Track operations and view folder/file stats.
- 🎨 **Stylish UI:** Modern design with colors, icons, and subtle animations.

---

## Prerequisites
To run SecureFolder, you’ll need:
- [Rust](https://www.rust-lang.org/tools/install) (latest version recommended)
- A terminal supporting colors and Unicode (e.g., iTerm2, Windows Terminal, or Linux terminals)

---

## Setup Instructions

1. Clone the repository:
   ```bash
   git clone https://github.com/jalalvandi/GuardX
   cd GuardX
   ```

2. Build the project:
   ```bash
   cargo build
   ```

3. Run the application:
   ```bash
   cargo run
   ```

## Usage

- Launch the app, and you’ll see a terminal-based interface. Use these controls to navigate and manage your files:

Controls
q: Quit the app
↑/↓: Move between folders or files
→/←: Switch between folder and file lists
k: Enter an encryption key
e: Encrypt the selected folder
d: Decrypt the selected folder
n: Create a new folder
p: Preview file contents
r: Remove a folder or file (with confirmation)
t: Open settings
i: Toggle dashboard and history
l: Load a saved key
v: Save the current key

## License
This project is licensed under the MIT License. See the LICENSE file for details (add one if it’s missing!).