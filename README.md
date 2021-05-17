# ya-runtime-sdk

`ya-runtime-sdk` is a Rust library for building new computation environments and self-contained runtimes for `yagna`,
executed by Provider nodes in the Golem network.

The crate provides a default implementation and customizable wrappers in the following areas:

- interfacing with the computation orchestrator
- runtime execution lifecycle
- runtime state management
- billing reports

---

Table of contents
- [Runtime overview](#runtime-overview)
  - [Runtime orchestration](#runtime-orchestration)
    - [Runtime execution mode](#runtime-execution-mode)
    - [Command execution mode](#command-execution-mode)
  - [Complementary functions](#complementary-functions)
  - [Events](#events)
- [Implementation](#implementation)
  - [Context](#context)
  - [Configuration](#configuration)
- [Debugging](#debugging)
- [Deploying](#deploying)

---

## Runtime overview

Runtimes are responsible for executing provisions of the Agreement between a Provider and a Requestor. Runtimes are
invoked only after a successful marketplace negotiation has taken place - the prices, deadlines and let hardware
resources have been agreed upon, and where both sides are able to communicate over the network via a common protocol.
Requestors are able to send ExeScripts to runtimes over the network, in order to control the flow of the computation.

Computation environments are fully and remotely controlled by Requestors. It's their responsibility to initialize the
deployment (configuration) phase of the payload specified in an Agreement, followed by starting (bringing up) the
environment, executing commands inside that environment and terminating the whole setup.

Self-contained runtimes on the other hand are built for specific use-cases, where the executed "payload" is 
pre-defined by the implementor (e.g. access to a locally running SQL server). Deploying, starting, executing 
commands (if implemented) and terminating the runtime can be only hinted by the Requestor; behaviour of those 
actions is fully determined by implementation.

### Runtime orchestration

Runtimes are a part of a logical building block called "ExeUnit". Runtime binaries are invoked by their orchestrating
part, the ExeUnit Supervisor, utilizing a standardized set of arguments and flags. `ya-runtime-sdk` handles all command
line interaction of the runtime binary - parses and validates the arguments to expose them later to the runtime
implementation via an execution context parameter, passed to each method of the main `Runtime` interface.

The `Runtime` interface provides means to customize application behaviour in each of execution phases:

- deployment

  Execute custom configuration steps.

- start

  Enable the runtime to be used by the Requestor.

  Can be executed in one of the [execution modes](#runtime-execution-mode).

- running a command

  Parse, interpret and handle an array of string arguments. Implementation is optional for self-contained runtimes.

  Can be executed in any of the [execution modes](#command-execution-mode)

- killing a running command

  Implementation is optional for self-contained runtimes.

- stop

  May be triggered by the Requestor or Provider Agent in a local provider setup, due to e.g. execution time constraints
  specified in the Agreement. Runtime shutdown is given a short (less than 5s) time window to perform a graceful
  shutdown, then the process is killed.

#### Runtime execution mode

There are 2 modes that a runtime can be executed in, defined by the developer as a constant `MODE` property of the 
main trait.

- `Server`

  The `start` command implementation is intended to be long-running and awaiting termination either via `stop` or
  receiving a signal by the process. Usually, `run` commands are expected to be invoked in `Server` mode (
  see [command execution modes](#command-execution-mode)).


- `Command`

  `start` command is a one-shot invocation, which returns promptly. All of the `run` commands are expected to be invoked
  via command line.

#### Command execution mode

The interface provides the mode the command is executed in as a parameter. The implementation should cover all execution
modes (`Server`, `Command`) but is not required to support all of them.

- `Server`

  When command is executed by a runtime running in a `Server` mode.

  The implementation MUST emit the following events of the command execution lifecycle:

    - command started
    - command output (if any)
    - command error output (if any)
    - command stopped

  Events above can be published via an `emmiter` property, a part of the context object passed to each function of the
  main interface.

- `Command`

  The `run` command was invoked via command line.

### Complementary functions

In addition to the lifecycle and execution logic, runtimes can implement 2 additional functions to customize their
behaviour:

- test

  Perform a self-test. Always executed during Provider Agent startup - i.e. during local yagna provider initialization,
  external to the negotiation phase and computation.

  Implementation must allow execution at any time.

  Example: the VM runtime (`ya-runtime-vm`) spawns a Virtual Machine with a minimal test image attached, to verify that
  KVM is properly configured on provider's operating system and all bundled components are available in the expected
  location on disk.


- offer

  Inject custom properties and / or constraints to an Offer, published by the Provider Agent on the marketplace. `offer`
  is executed by the Provider Agent while publishing new offers on the market, also during agent's initialization phase.

  Implementation must allow execution at any time.

### Events

Future versions of `ya-runtime-sdk` may cover following additional events:

- indication of runtime state change with a descriptor JSON object
- custom usage counters for billing purposes
- TBD

---

## Implementation

Runtimes can be implemented by performing the following steps:

    - `#[derive(Default, RuntimeDef)]` on a runtime struct
    - implement the `Runtime` trait for the struct
    - use the `ya_runtime_sdk::run` method to start the runtime

### Context

Each of the `Runtime` trait functions is parameterized with a mutable reference to the runtime object, and a runtime
execution context object.

The `Context` struct exposes the following properties:

- `cli`

  Command line arguments provided to the binary

- `conf`

  Deserialized runtime configuration (can be modified)

- `conf_path`

  Path to the configuration file on the local filesystem

- `emitter`

  Runtime event emitter

`Context` also exposes functions for configuration (de)serialization:

- `read_config`

  Read configuration from the specified path

- `write_config`

  Write configuration to the specified path

### Configuration

`ya-runtime-sdk` automatically deserializes configuration from JSON (default) / YAML / TOML files and provides means to
serialize and persist modified configuration to disk. Runtime configuration is customizable via `#[config(ConfStruct)]`
attribute of the `ServiceDef` derive macro and available as a property of the execution `Context`.

On start, the configuration is loaded from a file located at `~/.local/share/<crate_name>/<crate_name>.<format>` and
serialized to disk with default values when missing.

## Debugging

Runtimes running in `Server` execution mode can be interacted and debugged with
the [ya-runtime-dbg](https://github.com/golemfactory/ya-runtime-dbg) tool. See the README file in the linked repository
for more details.

## Deploying

**Note:** these instructions will soon become obsolete.

1. Build exe unit from the mf/self-contained branch (https://github.com/golemfactory/yagna/pull/1315)

    - copy the built binary to the plugins directory
    
2. Create a `ya-runtime-<runtime_name>.json` descriptor file in the plugins directory.

```json
    [
      {
        "name": "<wrapper_name>",
        "version": "0.1.0",
        "supervisor-path": "exe-unit",
        "runtime-path": "<wrapper_dir>/<wrapper_bin>",
        "description": "Service ",
        "extra-args": ["--runtime-managed-image"]
      }
    ]
```

3. Edit ~/.local/share/ya-provider/presets.json:
    
    - create a `presets` entry for `exeunit-name: "service_wrapper"` (e.g. copy an existing preset)
    - include the preset name in `active` array

4. Start `golemsp` or `ya-provider`.

The **plugins directory** is by default located at:

  - golemsp: `~/.local/lib/yagna/plugins`
  - ya-provider: `/usr/lib/yagna/plugins/`
