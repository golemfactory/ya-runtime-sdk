# ya-runtime-sdk

`ya-runtime-sdk` is a Rust library for building Computation Environments and Self-contained Runtimes, 
executed by Provider nodes in the Golem network.

`ya-runtime-sdk` provides facilitation in the following areas:

- interfacing with the computation orchestrator
- computation lifecycle management
- command output handling
- billing information reporting (TBD)

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

Runtime is responsible for performing computation specified in an Agreement between a Provider and a 
Requestor. Runtimes are executed only after a successful marketplace negotiation has taken place, where prices, 
computation deadlines and rented hardware resources have been agreed upon.

The degree of remote control that a Requestor can have over a Runtime depends on its flavour:

1. Computation Environment
   
   Fully controlled by the Requestor via a well-defined set of commands ("ExeScript"). 
   Requestor is responsible for triggering deployment of the payload (e.g. a Virtual Machine image) 
   specified in an Agreement, starting the environment, executing commands within that environment 
   and terminating the setup.


2. Self-contained Runtime
   
   The payload is pre-defined by the developer. In this case, deploying, starting, executing
   commands and terminating the runtime can be only hinted by the Requestor; the implementation determines the exact 
   behaviour of those actions.
    

### Runtime orchestration

Runtimes are orchestrated by their parent process, the ExeUnit Supervisor, via a standardized set of command line 
arguments and a Runtime API protocol. `ya-runtime-sdk` handles all the communication automatically for any struct 
implementing the `Runtime` trait.

The `Runtime` trait specifies handlers for each of the execution phases:

- `deploy`

  **Description:** Initial configuration of the environment and payload.
  
  **Output:** Optional JSON response in the following format:
  ```json 
  {
      "startMode": "blocking",
      "valid": {"Ok": "success"},
      "vols": [
          {"name": "vol-9a0c1c4a", "path": "/in"},
          {"name": "vol-a68672e0", "path": "/out"}
      ],
      "customKey": "customValue"
  }
  ```

  **Required properties**:
  - `startMode`
    
    Determines whether a runtime is a long-running application or a one-shot command.
    - `"blocking"` for `RuntimeMode::Server`
    - `"empty"` for `RuntimeMode::Command`
  
  - `valid`
    
    Deployment status.
    - `{"Ok": "success message"}`
    - `{"Err": "error message"}`
  
  - `vols`
    
    Local filesystem directory to runtime directory mapping.
    - `name` is a subdirectory on a local filesystem, in a directory chosen by the Supervisor,
    - `path` is an alias of that directory, seen from inside the runtime and Supervisor services (e.g. file transfer)
    
- `start`

  **Description:** Enable the runtime to be used by the Requestor.

  Can be run in **one** of the [execution modes](#runtime-execution-mode).
  
  **Output:** Optional JSON response.

- `run_command`

  **Description:** Parse, interpret and handle an array of string arguments. Optional in case of self-contained runtimes.

  Can be run in **any** of the [execution modes](#command-execution-mode)
  
- `kill_command`

  **Description:** Terminate command execution.

  Optional in case of self-contained runtimes.

- `stop`

  **Description:** Terminate the runtime.

  May be triggered by the Requestor on demand or by the Provider Agent due to execution time constraints
  specified in the Agreement. Runtime is given a short (< 5s) time window to perform a graceful shutdown, otherwise
  it will be forcefully closed.

#### Runtime execution mode

`Runtime::MODE` specifies one of 2 possible execution modes:

- `Server`

  The `start` command spawns a background task and returns promptly. Usually, `run` commands have to be invoked 
  in `Server` mode (see [command execution modes](#command-execution-mode)).


- `Command`

  `start` command is a one-shot invocation, which completes promptly. All of the `run` commands are expected to be invoked
  via command line.

#### Command execution mode

- `Server`

  Command was invoked by a runtime running in a `Server` mode.

  The implementation **MUST** use the `emitter` to publish the following command lifecycle events:

    - command started
    - command output (if any)
    - command error output (if any)
    - command stopped

  Developers may use the `RunCommandExt` trait to wrap the lifecycle and publish output events in a simpler manner.  

- `Command`

  Command was invoked via command line.

`run_command` implementation should distinguish each of the execution modes but is not required to support both.

### Complementary functions

In addition to the lifecycle and execution logic, runtimes can implement 2 additional customization functions:

- `test`

  **Description:** Perform a self-test. 
  
  Always executed during Provider Agent startup - i.e. during local yagna provider initialization,
  external to the negotiation phase and computation.

  Implementation must allow execution at any time.

  **Example:** the VM runtime (`ya-runtime-vm`) spawns a Virtual Machine with a minimal test image attached, to verify that
  KVM is properly configured on provider's operating system and all bundled components are available in the expected
  location on disk.

- `offer`

  **Description:** Inject custom properties and / or constraints to an Offer, published by the Provider Agent on the marketplace. `offer`
  is executed by the Provider Agent while publishing new offers on the market, also during agent's initialization phase.

  Implementation must allow execution at any time.

  **Output:** Optional JSON response in the following format:

  ```json 
  {
      "constraints": "",
      "properties": {}
  }
  ```

### Events

(TBD) Future versions of `ya-runtime-sdk` may cover additional types of events:

- runtime state change indication w/ JSON payload
- custom usage counters for billing purposes

---

## Implementation

Runtimes can be implemented by performing the following steps:

  - `#[derive(Default, RuntimeDef)]` on a runtime struct
  - implement the `Runtime` trait for the struct
  - use the `ya_runtime_sdk::run` method to start the runtime

See the [`example-runtime`](examples/example-runtime) for more details.

### Context

Each of the `Runtime` trait functions is parameterized with a mutable reference to the runtime, and a runtime
execution context object.

The `Context` struct exposes the following properties:

- `cli`

  Command line arguments provided to the binary

- `conf`

  Deserialized runtime configuration

- `conf_path`

  Path to the configuration file on a local filesystem

- `emitter`

  Command event emitter

`Context` also exposes functions for configuration persistence:

- `read_config`

  Read configuration from the specified path. Supports `JSON` (default), `YAML` and `TOML` file formats.
  The desired format is 

- `write_config`

  Write configuration to the specified path.

### Configuration

Configuration struct can be set via a `#[config(..)]` attribute of the `RuntimeDef` derive macro. On runtime startup, 
configuration is read from a file located at `~/.local/share/<crate_name>/<crate_name>.<format>` and
serialized to disk with default values when missing.

## Debugging

Developers can use the [ya-runtime-dbg](https://github.com/golemfactory/ya-runtime-dbg) tool to interact with a runtime
running in `Server` mode. See the `README.md` file in the linked repository for more details.

## Deploying

1. Create a `ya-runtime-<runtime_name>.json` descriptor file in the **plugins directory**.

```json
  [
    {
      "name": "<runtime_name>",
      "version": "0.1.0",
      "supervisor-path": "exe-unit",
      "runtime-path": "<runtime_dir>/<runtime_bin>",
      "description": "Custom runtime ",
      "extra-args": ["--runtime-managed-image"]
    }
  ]
```

2. Edit `~/.local/share/ya-provider/presets.json`:
    
    - create a `presets` entry for `exeunit-name: "<runtime_name>"` (e.g. copy an existing preset)
    - include the preset name in `active` array

3. Start `golemsp` or `ya-provider`.

The **plugins directory** is by default located at:

  - golemsp: `~/.local/lib/yagna/plugins/`
  - ya-provider: `/usr/lib/yagna/plugins/`
