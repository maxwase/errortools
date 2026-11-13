# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.1.0] - 2026-11-13

### Added

- `Code`: the trait an error implements to be sent over gRPC. Every method has a default, so an empty impl maps the error to `tonic::Code::Internal`; a type overrides the status code, opts in to sending its `Suggest` hint, and exposes a `ManyErrors` it holds.
- `IntoStatus` and `Mapping`: error to `tonic::Status`. The default mapping sends the code, the top-level message, the hint and the top-level message of each aggregate leaf; no source text leaves the server. `Mapping::detailed()` (`Detail::Chain`) adds the whole source chain as the `OneLine` message and a `google.rpc.DebugInfo` detail.
- `ManyErrors` implements `Code`: it maps to `InvalidArgument` and its leaves become `google.rpc.BadRequest` field violations, with group labels as a dot-separated field prefix. `detach_leaves` does the same for an aggregate held by a wrapper type.
- `Mapping::with_tree`: the opt-in binary trailer `errortools-tree-bin` (`TREE_TRAILER`) that keeps the nested structure of an aggregate. It is capped at 4 KiB on the wire by default (`DEFAULT_TREE_CAP`) and is dropped, never truncated, when it does not fit.
- `ErrorStatusExt`: `tonic::Status` back to errortools types on the client. `to_detached` rebuilds the chain as a `Detached`, `to_many_errors` rebuilds the aggregate as `DetachedErrors`, and `hint` reads the hint.
- Limits on what a client accepts from a server: groups nest at most 32 levels deep and a chain holds at most 1024 messages, so received bytes cannot exhaust the stack.
