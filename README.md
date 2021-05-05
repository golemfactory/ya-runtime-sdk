# ya-service-sdk

In order to implement a custom service, perform the following steps:

- `#[derive(Default, ServiceDef)]` on a service struct
- implement the `Service` trait
- use the `service_sdk::run` method to start the service

## Example

See the [example-service](https://github.com/golemfactory/ya-service-sdk/blob/main/examples/example-service/src/main.rs) project.

## Context

Each of the `Service` trait functions is parameterized with a mutable reference to the service object, and a service execution context object.

The `Context` struct contains following properties:

- `cli` - command line arguments provided to the binary
- `conf` - service configuration
- `conf_path` - configuration file path on the local filesystem
- `emitter` - a service event emitter (see [Events](#events))


## Configuration

One can define custom service configuration by

- deriving `Default, Serialize, Deserialize` on the configuration struct
- decorating the service struct with `#[conf(<struct_name>)]`.

Configuration is deserialized on start from `~/.local/share/<crate_name>/<crate_name>.json` and serialized to disk with default values if missing.

## CLI

One can extend the default provided CLI by

- deriving `StructOpt` on the CLI struct
- decorating the service struct with `#[cli(<struct_name>)]`

Custom CLI arguments will be injected to the main CLI struct as can be seen here: https://github.com/golemfactory/ya-service-sdk/blob/main/derive/src/lib.rs#L71

## ServiceMode

Code & comments: https://github.com/golemfactory/ya-service-sdk/blob/main/ya-service-sdk/src/runner.rs#L89-L99

By default, services start in `ServiceMode::Server` mode which enforces the use of Runtime API and a blocking implementation of `Service::start`. It's possible to change the mode by setting `const MODE: ServiceMode = ServiceMode::Command;` in `impl Service for <service_struct>`

## Events 

Event `emitter` is a property of the `Context` param of each function of the Service trait. Currently it's possible to publish 4 kinds of events when running in `ServiceMode::Server`:

- command started
- command stopped
- command stdout
- command stderr

All events above are processed and buffered by the ExeUnit Supervisor. `Started` and `Stopped` events **MUST** be emitted when implementing `Service::run_command` (`RUN` in ExeScript dictionary).
