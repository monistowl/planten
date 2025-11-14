#PLAN TEN: the Rust 9front

planten is basically “What if Plan 9 had been born in Rust and grew up with 9front?” so I’ll structure this as:

* Big-picture goals & constraints
* Architecture & design decisions
* Compatibility & “piece-by-piece from 9front” strategy
* Concrete roadmap (phased, with milestones)
* Rust ecosystem choices & internal crate layout
* Tooling, testing, and project management
* Major risks and ways to keep it sane

You can find good packages to adapt at https://git.9front.org/

---

## 1. Vision, Goals, and Non-Goals

### 1.1 Core Vision

planten = a from-scratch Rust implementation of the Plan 9 / 9front world:

* **Per-process namespaces as first-class citizens** – every process has its own file-tree view, built with `bind`/`mount` semantics. ([9p][1])
* **Everything-is-9P** – 9P2000 (or a close kin) as the canonical interface to *everything*: devices, services, GUIs, networking, auth. ([MIT CSAIL][2])
* **9front compatibility** – speak the same protocols, mimic the same userland and layout enough that you can join a 9front cluster, mount planten services from 9front, etc. ([9front Git][3])
* **Rust-native** – use Rust to improve memory safety and reliability over the original C codebase (taking lessons from Redox and other Rust OSes). ([LWN.net][4])

### 1.2 Explicit Goals

* Bootable, bare-metal **x86-64** kernel with Plan9-ish environment (later: ARM64/RISC-V).
* **9P2000(-ish) wire compatibility** with 9front, including authentication.
* Per-process namespaces, union mounts, `/proc` and `/net` implemented as file servers. ([Wikipedia][5])
* A Rust rewrite of the *essence* of:

  * kernel + filesystems
  * `rio`-like window system and a basic Acme-ish environment
  * core userland (`rc`, `ls`, `cat`, `bind`, `mount`, `/bin` layout)
  * networking stack & `ip` suite (as 9P-served /net)
* Modern build toolchain: `cargo`-driven, QEMU targets, cross-compilation.

### 1.3 Non-Goals (at least early on)

* **POSIX compatibility** (beyond what’s trivial). Plan 9/9front are explicitly non-POSIX; copying Linux ABI would dilute the design. ([Wikipedia][5])
* **Binary compatibility** with 9front. Target: source-level and protocol-level compatibility.
* **Desktop-first** polish. The focus is a coherent distributed OS, not yet-another Linux desktop.
* **Security panacea**: Rust helps, but Plan 9/9front’s security model is quirky; you’ll still need careful threat modeling.

---

## 2. Architectural Overview

### 2.1 Kernel Model

Borrow heavily from Plan 9 concepts but use a Rust microkernel-ish style similar in spirit to Redox (not identical):

* **Small core kernel**:

  * CPU management, scheduling, interrupts
  * Virtual memory
  * Physical memory management
  * Basic synchronization primitives
  * Process and thread management
* **Everything else as servers over 9P**:

  * `rootfs` and other filesystems
  * `/dev`-like device trees
  * `/net` networking stack
  * `/proc` process control interface
  * GUI/window system namespaces

Rust design choices:

* `no_std` for the kernel, with a minimal `core`-only subset.
* Strict separation of `unsafe` code to small HAL/driver modules.
* Strongly-typed kernel objects (PIDs, FDs, FIDs, etc.) to avoid confusion.

### 2.2 Namespaces and 9P

Core Plan 9 abstractions to preserve:

* **Per-process namespace** – each process has a mount table defining its view of the world; `bind` and `mount` operations splice 9P trees together. ([Wikipedia][5])
* **Union mounts** – directory unions for `/bin`, `/srv`, etc., with ordering semantics (before / after) mapped to clear Rust data structures. ([Wikipedia][5])
* **9P2000 protocol** as the lingua franca:

  * Kernel exports 9P endpoints for local servers.
  * Network 9P used for remote resources.

Internally:

* Shared `planten_9p` crate implementing the protocol and message types.
* Async/event-driven 9P “mux” that can connect local and remote servers uniformly.

### 2.3 Device Model and Virtual Filesystems

Plan 9 uses filesystems to expose devices (`/dev`, `/proc`, `/net`, etc.). ([Wikipedia][5])

planten should:

* Implement **devices as 9P file server processes** (some in kernel, some in privileged userland, but *always via 9P*).

* Provide a unified device trait:

  ```rust
  trait FsServer {
      fn walk(&self, ...);
      fn open(&self, ...);
      fn read(&self, ...);
      fn write(&self, ...);
      fn clunk(&self, ...);
      // ...
  }
  ```

* Make `/proc` and `/net` first 9P pseudo-filesystems implemented in Rust.

### 2.4 Userspace Model

* **Single address-space style** is not required; you can still do standard process isolation with address spaces but keep the *illusion* of a unified namespace.
* Binary format: Plan 9 uses its own a.out variant; you can:

  * Start with a simple ELF or custom format.
  * Provide a toolchain to build native planten binaries.
* `rc` shell reimplementation in Rust (possibly with a small interpreter for Plan 9 rc scripts).

### 2.5 GUI / rio / acme

* Implement a minimal `rio`-like window system as:

  * A 9P server exporting per-window file trees.
  * An event loop on top of framebuffer + input devices.
* `acme`-like environment:

  * Possibly as a later-phase project: minimal, focusing on plumbing via /srv and /dev interfaces.

---

## 3. Compatibility & “Piece-by-Piece from 9front”

### 3.1 Target Compatibility

You want to be able to:

* Boot planten as a **CPU server**, accessed from a 9front terminal.
* Run planten as a **file server** exported via 9P to a 9front network.
* Eventually run planten-native `rio` and tools, but still be able to mount 9front file servers.

9front-specific extensions to consider:

* **Extra drivers and x86-64 support** – copy coverage, not implementation details. ([The Register][6])
* **Improved protocols and auth** (e.g., dp9ik, TLS-ish tunnels). ([9front Git][7])

### 3.2 Strategy: “Semantic Porting”

Rather than transliterating C → Rust line-by-line, do:

1. Extract **behavioural spec** from 9front:

   * For each subsystem, write a short design doc: “What does this look like to a user or another program?”
   * Use 9front source + manuals + FQAs as reference. ([9front Git][3])

2. Implement that spec in Rust idioms:

   * Memory-safe data structures.
   * Strong typing for protocol fields and flags.
   * Error types instead of error codes.

3. Build a **compatibility test suite**:

   * For 9P: capture traces of 9front clients talking to servers and replay them against planten servers.
   * For core tools: script behaviour and compare outputs.

---

## 4. Roadmap: Phases and Milestones

I’ll give you a staged roadmap that always tries to keep *something usable* at each stage, even if it’s “Plan 9 semantics on top of Linux”.

### Phase 0 – Foundation & Spec Extraction

**Goals:**

* Understand and document the Plan 9/9front world you’re recreating.
* Choose the low-level Rust stack and targets.

**Tasks:**

* Read/annotate key Plan 9 papers:

  * Pike’s “Plan 9 from Bell Labs” overview ([USENIX][8])
  * “The Use of Name Spaces in Plan 9” ([9p][1])
* Document:

  * Namespace operations (bind, mount, union rules).
  * 9P message set and semantics.
  * Layout of `/`, `/dev`, `/proc`, `/net`, `/srv`, `/tmp`, etc.
  * 9front-specific deltas (e.g. new file servers, protocols). ([GitHub][9])
* Select *initial* hardware target: x86-64 PC with UEFI boot.
* Bootstrap repo structure (see Section 5).

**Deliverables:**

* `docs/architecture.md` – high-level design.
* `docs/compatibility-matrix.md` – 9front features vs planned planten features.
* First prototype crate `planten_9p` implementing 9P types and message encode/decode.

---

### Phase 1 – 9P and Namespace on Top of Unix (“Plan 9 as a library”)

Before doing a kernel, build a user-space sandbox:

**Goals:**

* Implement 9P2000 protocol and per-process-like namespaces **on top of Linux/BSD** in Rust.
* Have a minimal `bind`/`mount` implementation and a `namespaced` process launcher.

**Tasks:**

* `planten_9p`:

  * Complete encode/decode for all 9P2000 messages.
  * Add a test harness with golden binary frames.
* `planten_ns` crate:

  * Model namespace as a tree of 9P mounts (with union support).
  * CLI tools: `bind`, `mount`, `nsctl`, and a `10_ns` launcher that starts a process in a composed namespace.
* Implement a few toy 9P servers:

  * in-memory filesystem
  * a proc-lite server exposing a fake `/proc` for debugging.
* Integrate with `sshfs`-style remote 9P over TCP for dev/testing.

**Deliverables:**

* Run `ls` inside a Rust userland tool that sees a virtual file tree composed by 9P mounts.
* Tests verifying union semantics match Plan 9 rules (e.g. duplicate names, search order). ([Wikipedia][5])

---

### Phase 2 – Minimal Rust Kernel & Boot Flow

**Goals:**

* Boot a Rust kernel on bare metal (or QEMU) that can:

  * Initialise hardware
  * Set up paging
  * Create a basic process
  * Run a small userland program

**Tasks:**

* Create `planten_kernel`:

  * Bootloader/early init (with `limine` or hand-rolled boot stub).
  * Interrupt descriptor tables, timer, basic scheduler.
  * Physical and virtual memory management.
  * Basic system call / trap mechanism, *even if in the final form most IPC is 9P*.
* Provide minimal “root” filesystem:

  * Initially: a RAMFS populated at boot.
  * Later replaced with a 9P file server.
* Implement a minimal userland binary loader (ELF or custom).

**Deliverables:**

* QEMU boot that drops you to a primitive shell or prints logs.
* Basic process creation and context switching.

---

### Phase 3 – Kernel-Integrated 9P and Namespaces

**Goals:**

* Make 9P the primary interface between the kernel and all higher-level functionality.
* Implement per-process namespaces in the kernel.

**Tasks:**

* Integrate `planten_9p` in-kernel:

  * Provide kernel-side 9P client and server.
* Implement:

  * **Mount table per process**.
  * `bind` and `mount` syscalls.
  * A root `srv`-style registry of file servers.
* Implement core pseudo-filesystems as 9P servers:

  * `/dev` – consoles, random, null, kernel logs.
  * `/proc` – each process as directory, with ctl/status files. ([Wikipedia][5])
* Implement `rc`-like init userland that:

  * Mounts these servers into a coherent namespace.
  * Starts a shell on the console.

**Deliverables:**

* Boot planten kernel → see `/dev`, `/proc` exported as 9P tree, accessible from userland.
* `bind`/`mount` working for local pseudo-filesystems.

---

### Phase 4 – Storage, File Servers, and Networking

**Goals:**

* Persistent filesystem.
* Networking stack accessed via `/net` and 9P. ([Wikipedia][5])

**Tasks:**

* Implement a basic disk filesystem:

  * Initially, a simple log-structured or journaling filesystem (inspired by Plan 9’s vnode model).
  * Add 9P server for the disk filesystem as root FS.
* Networking:

  * Implement an IP stack (possibly factor from an existing Rust project if licensing allows).
  * Map sockets to `/net`:

    * `/net/tcp`
    * `/net/udp`
    * etc.
  * Expose connect/listen operations via file writes/read semantics (like Plan 9). ([Wikipedia][5])
* Remote 9P mounts:

  * planten as a **client** of 9front file servers.
  * planten as a **server** (export local namespaces to remote clients).

**Deliverables:**

* Boot planten, configure network, and `mount` a 9front 9P service into the local namespace.
* Remote `ls` over the network via 9P.

---

### Phase 5 – Auth, Users, and Security Model

**Goals:**

* Provide a coherent authentication model similar to Plan 9 / 9front (e.g. `factotum`, `dp9ik`). ([9front Git][7])

**Tasks:**

* Implement a small credential service:

  * A `factotum`-style 9P server storing keys and performing protocol handshakes on behalf of other processes.
* User identities and permissions:

  * Implement user IDs in kernel and per-process credentials.
  * Map them to file ownership semantics.
* Integrate authentication into 9P:

  * `auth` messages
  * Challenge-response protocols
* CLI tools for managing keys, identities, logins to remote resources.

**Deliverables:**

* Ability to mount a protected 9P share from 9front using planten’s auth stack.
* `who`, `id`, and basic access control enforcement.

---

### Phase 6 – rio-like Window System and Basic GUI

**Goals:**

* Implement a rio-esque window system that:

  * Exports per-window `/dev`-like files.
  * Uses Plan 9’s mouse/keyboard/graphics device conventions. ([Wikipedia][5])

**Tasks:**

* Framebuffer and input drivers (kernel or privileged userland).
* `rio10` server:

  * Expose `/dev/mouse`, `/dev/cons`, `/dev/draw` equivalents per window.
  * Window creation via file interfaces.
* Terminal emulator windows running `rc`:

  * Use same semantics as 9front where possible.
* Optional: simple `acme10` environment later, composed similarly via 9P.

**Deliverables:**

* Boot to a graphical environment where:

  * `rio10` runs.
  * You can open windows, run `rc`, and use basic commands with proper namespaces.

---

### Phase 7 – Porting and Reimplementing Key 9front Userland

**Goals:**

* Have enough userland to be self-hosting (build the system from itself).
* Provide a recognisably 9front-ish CLI.

**Targets:**

* Shells and scripting:

  * `rc` (core).
* Core utilities:

  * `ls`, `cat`, `cp`, `mv`, `grep`, `awk`-like tool, `sed`-like tool.
* Build tools:

  * Rust toolchain cross-compiled to planten.
  * `mk` reimplementation in Rust (or compatibility layer).
* Networking tools:

  * `ip`, `dns`, `telnet`, `ssh`-like (if desired), `cpu`-like remote execution (Plan 9 style). ([Wikipedia][5])

**Deliverables:**

* planten can compile its own kernel and userland in a mostly self-contained way.
* You can develop and run non-trivial software entirely inside planten.

---

### Phase 8 – Distributed System Features and CPU/File Servers

**Goals:**

* Realise the “cluster as one machine” idea in the planten world. ([Wikipedia][5])

**Tasks:**

* CPU server:

  * Implement `cpu`-like remote execution: export the local terminal devices to a remote CPU server; run processes there, but I/O here.
* File server:

  * Dedicated high-performance file server nodes (possibly with different kernel config).
* Dynamic namespace composition tools:

  * Scripts and tools to assemble per-user and per-service namespaces across multiple machines.

**Deliverables:**

* Multi-node planten + 9front cluster where:

  * Users log into planten terminal.
  * CPU-intensive work runs on remote CPU servers.
  * Data lives on planten file servers.
  * Namespaces hide the distribution.

---

### Phase 9 – Polishing, Tooling, and Experiments

**Goals:**

* Clean up APIs, documentation, and developer ergonomics.
* Explore new directions that Plan 9 never quite got to.

**Ideas:**

* **Safer driver framework**: trait-based, hot-swappable drivers with test harnesses.
* **Sandboxing**:

  * Capability-like controls on 9P access.
  * Explicit per-process or per-session restrictions on bind/mount.
* **Integration with modern tooling**:

  * Enhanced acme/rio for code navigation (Rust-aware).
  * LSP-style tooling served over 9P.

---

## 5. Rust Crate & Repo Layout

A plausible workspace structure:

* `kernel/`

  * `planten_kernel` – core kernel crate.
  * `planten_hal` – architecture and platform-specific HAL (x86-64, later others).
  * `planten_drivers_*` – group drivers by class (block, net, input, etc.).
* `libs/`

  * `planten_9p` – protocol (shared by kernel and userland).
  * `planten_fs_core` – common filesystem traits and utilities.
  * `planten_fs_ramfs`, `planten_fs_diskfs`, `planten_fs_proc`, `planten_fs_net`, etc.
  * `planten_ns` – namespace and union mount logic.
* `userspace/`

  * `planten_rc` – rc shell.
  * `planten_coreutils` – basic utilities.
  * `planten_rio` – window system.
  * `planten_acme` – optional later.
  * `planten_factotum` – auth agent.
* `tools/`

  * `planten_qemu_runner` – dev runner for QEMU with appropriate args.
  * `planten_image_builder` – create bootable disk images.
* `docs/` – architecture, design notes, compatibility matrix.

Use a Cargo workspace at the top-level to coordinate builds.

---

## 6. Tooling, Testing, and CI

### 6.1 Testing Strategy

* **Unit tests** in Rust for pure logic (9P encode/decode, namespace operations).
* **Golden tests**:

  * Capture 9front 9P traffic and replay against planten servers to check compatibility.
* **Integration tests via QEMU**:

  * Boot the OS in CI.
  * Run scripted tests in a virtual serial console (`expect`-style).
* **Property-based tests**:

  * For filesystem operations, 9P sequences, etc.

### 6.2 Dev Environment

* Cross-compile from Linux (or macOS if you must suffer).
* Use `qemu-system-x86_64` for fast iteration.
* Debugging:

  * QEMU + GDB stub.
  * Logging via serial port early on.

---

## 7. Risk Areas and Mitigations

### 7.1 Scope Explosion

This project is *huge*. To avoid getting lost:

* Keep a small, core “Minimal Plan 9” feature set:

  * Boot
  * 9P
  * `/dev`, `/proc`, `/net`
  * Basic userland
* Everything else gated behind explicit roadmap items.

### 7.2 Compatibility Drift

You want to be Plan 9/9front-ish, not “RustOS with a 9P veneer”.

Mitigations:

* Maintain **compatibility tests** against a running 9front system.
* Keep `docs/compatibility-matrix.md` honest and updated.
* Resist the temptation to “improve” semantics unless:

  * It’s optional, or
  * You clearly version the interface.

### 7.3 Rust Complexity / Unsafe Bloat

* Centralise all `unsafe` code in HAL and low-level driver crates.
* Use `#![forbid(unsafe_code)]` in high-level crates to catch leakage.
* Regularly audit kernel unsafe blocks.

---

## 8. Long-Range Extensions (Post-MVP)

Once you’ve got the core system up:

* **RISC-V and ARM64 ports** – align with modern hardware trends.
* **Modern packaging** – a 9P-served “pkg” tree for distributing binaries.
* **Mixed-world environments**:

  * planten as a Plan 9-like “overlay” on Linux via FUSE/9P.
  * planten servers exporting services to Linux and BSD.

---

[1]: https://9p.io/sys/doc/names.html?utm_source=chatgpt.com "The Use of Name Spaces in Plan 9"
[2]: https://css.csail.mit.edu/6.824/2014/papers/plan9.pdf?utm_source=chatgpt.com "Plan 9 from Bell Labs"
[3]: https://git.9front.org/plan9front/plan9front/HEAD/info.html?utm_source=chatgpt.com "plan9front - git"
[4]: https://lwn.net/Articles/979524/?utm_source=chatgpt.com "Redox: An operating system in Rust"
[5]: https://en.wikipedia.org/wiki/Plan_9_from_Bell_Labs?utm_source=chatgpt.com "Plan 9 from Bell Labs"
[6]: https://www.theregister.com/2022/11/02/plan_9_fork_9front/?utm_source=chatgpt.com "New version of Plan 9 fork 9front released"
[7]: https://git.9front.org/static/guide.html?utm_source=chatgpt.com "user guide - git"
[8]: https://www.usenix.org/publications/compsystems/1995/sum_pike.pdf?utm_source=chatgpt.com "Plan 9 from BeII Labs"
[9]: https://github.com/henesy/awesome-plan9?utm_source=chatgpt.com "henesy/awesome-plan9: A curated list of ..."

