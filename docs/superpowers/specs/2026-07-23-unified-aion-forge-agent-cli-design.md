# Unified Aion Forge Agent CLI Design

## Goal

Make Aion Forge discoverable and usable as a standard AionUI Agent without a per-machine command override. The installed canonical executable is `aion-forge`; the legacy `aion-forge-cli` command remains available as a compatibility alias.

## Current Problem

The installed `aion-forge-cli` executable exposes direct tool execution and an MCP server, while ACP support lives in the separately built `aion-forge-acp` executable. Release artifacts and installers do not install the ACP executable. AionUI therefore has an Aion Forge catalog entry but cannot find an installed standard Agent CLI and reports it as missing.

## Command Contract

The canonical executable supports `acp`, `mcp-server`, `setup`, direct tool execution, tool listing, help, and version entry points. ACP and MCP use stdout exclusively for protocol messages and stderr for diagnostics.

The compatibility command `aion-forge-cli` exposes the same command contract and version as `aion-forge`.

## Architecture

ACP protocol handling becomes reusable library functionality instead of being owned only by the `aion-forge-acp` binary. The unified CLI dispatches `acp` directly to that library and does not spawn a second executable.

Direct tool execution, MCP serving, setup, and ACP serving remain separate handlers behind one CLI parser. Shared initialization runs once before dispatch and preserves protocol stdout isolation.

## Packaging and Installation

Release builds produce `aion-forge` for every supported platform. Packages also contain `aion-forge-cli` as a compatibility alias of the same build and continue to contain `aion-server`.

Installers place both CLI names in the same user PATH directory. They do not write an AionUI command override. MCP setup registers `aion-forge mcp-server` through AionUI supported configuration helper.

## AionUI Discovery

AionUI discovers the Agent through `aion-forge` on PATH and starts `aion-forge acp`. No absolute development path, local command override, custom Agent record, or AionUI catalog modification is part of the solution.

## Compatibility

Existing scripts using `aion-forge-cli` continue to work. New documentation and configuration use `aion-forge`. Existing MCP configurations invoking `aion-forge-cli mcp-server` remain valid during the compatibility period.

## Test Strategy

Contract tests cover canonical help and version behavior, all command dispatch paths, the compatibility alias, ACP and MCP initialization with stdout purity, release artifact names, package contents, installer destinations, PATH behavior, and generated configuration defaults.

Implementation follows red-green-refactor for each changed boundary.

## Success Criteria

1. A fresh installation places `aion-forge` on PATH.
2. Standard help and version commands exit successfully without blocking.
3. `aion-forge acp` completes the AionUI ACP connection test.
4. AionUI marks Aion Forge installed without a command override.
5. MCP and direct tool behavior remain available.
6. Existing `aion-forge-cli` commands remain functional.
7. Focused contracts, workspace tests, formatting, and lint checks pass.

## Non-Goals

- Changing AionUI Agent detection implementation.
- Adding a custom Agent or absolute-path override.
- Changing provider credentials or model selection.
- Refactoring unrelated Forge crates.
