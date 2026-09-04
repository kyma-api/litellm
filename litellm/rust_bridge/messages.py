"""Thin Python wrapper for the native Rust Anthropic Messages bridge."""

from __future__ import annotations

from collections.abc import Awaitable
from dataclasses import dataclass
from typing import Final, Protocol, cast

import httpx

from litellm.rust_bridge.configuration import rust_enabled
from litellm.rust_bridge.runtime import (
    BridgeErrorContext,
    RustBridge,
    async_none,
    identity,
)
from litellm.rust_bridge.timeouts import timeout_to_seconds


class RustMessages(Protocol):
    def __call__(
        self,
        model: str,
        body: dict[str, object],
        api_key: str | None,
        api_base: str | None,
        custom_llm_provider: str | None,
        extra_headers: dict[str, object] | None,
        timeout_seconds: float | None,
    ) -> dict[str, object]:
        raise NotImplementedError


class RustAmessages(Protocol):
    def __call__(
        self,
        model: str,
        body: dict[str, object],
        api_key: str | None,
        api_base: str | None,
        custom_llm_provider: str | None,
        extra_headers: dict[str, object] | None,
        timeout_seconds: float | None,
    ) -> Awaitable[dict[str, object]]:
        raise NotImplementedError


class _Unset:
    pass


_UNSET: Final[_Unset] = _Unset()


@dataclass(slots=True)
class _RustMessagesState:
    messages: RustMessages | None = None
    amessages: RustAmessages | None = None


_STATE: Final[_RustMessagesState] = _RustMessagesState()


def set_rust_messages(
    *,
    messages: RustMessages | None | _Unset = _UNSET,
    amessages: RustAmessages | None | _Unset = _UNSET,
) -> None:
    if not isinstance(messages, _Unset):
        _STATE.messages = messages
    if not isinstance(amessages, _Unset):
        _STATE.amessages = amessages


def load_rust_messages() -> RustMessages | None:
    if _STATE.messages is not None:
        return _STATE.messages
    from litellm.rust_bridge import get_native_bridge

    native_bridge: Final = get_native_bridge()
    if native_bridge is None:
        return None
    return cast(RustMessages, getattr(native_bridge, "messages", None))


def load_rust_amessages() -> RustAmessages | None:
    if _STATE.amessages is not None:
        return _STATE.amessages
    from litellm.rust_bridge import get_native_bridge

    native_bridge: Final = get_native_bridge()
    if native_bridge is None:
        return None
    return cast(RustAmessages, getattr(native_bridge, "amessages", None))


_MESSAGES_ROUTE: Final = RustBridge(
    route="messages",
    load=lambda: load_rust_messages(),
    enabled=rust_enabled,
)
_AMESSAGES_ROUTE: Final = RustBridge(
    route="messages",
    load=lambda: load_rust_amessages(),
    enabled=rust_enabled,
)


def messages(
    *,
    model: str,
    body: dict[str, object],
    api_key: str | None,
    api_base: str | None,
    custom_llm_provider: str | None,
    extra_headers: dict[str, object] | None,
    timeout: float | httpx.Timeout | None,
    request_override: bool | None = None,
) -> dict[str, object] | None:
    return _MESSAGES_ROUTE.invoke(
        call=lambda rust_messages: rust_messages(
            model=model,
            body=body,
            api_key=api_key,
            api_base=api_base,
            custom_llm_provider=custom_llm_provider,
            extra_headers=extra_headers,
            timeout_seconds=timeout_to_seconds(timeout),
        ),
        fallback=lambda: None,
        adapt=identity,
        context=BridgeErrorContext(provider=custom_llm_provider or "", model=model),
        request_override=request_override,
    )


async def amessages(
    *,
    model: str,
    body: dict[str, object],
    api_key: str | None,
    api_base: str | None,
    custom_llm_provider: str | None,
    extra_headers: dict[str, object] | None,
    timeout: float | httpx.Timeout | None,
    request_override: bool | None = None,
) -> dict[str, object] | None:
    return await _AMESSAGES_ROUTE.ainvoke(
        call=lambda rust_amessages: rust_amessages(
            model=model,
            body=body,
            api_key=api_key,
            api_base=api_base,
            custom_llm_provider=custom_llm_provider,
            extra_headers=extra_headers,
            timeout_seconds=timeout_to_seconds(timeout),
        ),
        fallback=async_none,
        adapt=identity,
        context=BridgeErrorContext(provider=custom_llm_provider or "", model=model),
        request_override=request_override,
    )
