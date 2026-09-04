from __future__ import annotations

from collections.abc import Awaitable, Callable
from dataclasses import dataclass
from enum import Enum
from typing import Final, Generic, NoReturn, Protocol, TypeAlias, TypeVar

from litellm.exceptions import APIError
from litellm.rust_bridge.bindings import native_exception_types

BindingT = TypeVar("BindingT")
NativeT = TypeVar("NativeT")
ResultT = TypeVar("ResultT")


class HandoffReason(Enum):
    DISABLED = "disabled"
    UNAVAILABLE = "unavailable"
    DECLINED = "declined"


@dataclass(frozen=True, slots=True)
class RustHandled(Generic[ResultT]):
    value: ResultT


@dataclass(frozen=True, slots=True)
class RustHandoff:
    reason: HandoffReason
    detail: str | None = None


RustAttempt: TypeAlias = RustHandled[ResultT] | RustHandoff


@dataclass(frozen=True, slots=True)
class BridgeErrorContext:
    provider: str
    model: str


class RustEnablement(Protocol):
    def __call__(self, *, request_override: bool | None = None) -> bool: ...


@dataclass(frozen=True, slots=True)
class RustBridge(Generic[BindingT]):
    route: str
    load: Callable[[], BindingT | None]
    enabled: RustEnablement

    def _attempt(
        self,
        *,
        call: Callable[[BindingT], NativeT],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
        request_override: bool | None = None,
        eligible: bool = True,
    ) -> RustAttempt[ResultT]:
        binding_or_handoff: Final = self._binding_or_handoff(
            request_override=request_override,
            eligible=eligible,
        )
        if isinstance(binding_or_handoff, RustHandoff):
            return binding_or_handoff
        return self._attempt_call(
            call=lambda: call(binding_or_handoff),
            adapt=adapt,
            context=context,
        )

    async def _aattempt(
        self,
        *,
        call: Callable[[BindingT], Awaitable[NativeT]],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
        request_override: bool | None = None,
        eligible: bool = True,
    ) -> RustAttempt[ResultT]:
        binding_or_handoff: Final = self._binding_or_handoff(
            request_override=request_override,
            eligible=eligible,
        )
        if isinstance(binding_or_handoff, RustHandoff):
            return binding_or_handoff
        return await self._attempt_acall(
            call=lambda: call(binding_or_handoff),
            adapt=adapt,
            context=context,
        )

    def invoke(
        self,
        *,
        call: Callable[[BindingT], NativeT],
        fallback: Callable[[], ResultT],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
        request_override: bool | None = None,
        eligible: bool = True,
    ) -> ResultT:
        result: Final = self._attempt(
            call=call,
            adapt=adapt,
            context=context,
            request_override=request_override,
            eligible=eligible,
        )
        if isinstance(result, RustHandled):
            return result.value
        return fallback()

    async def ainvoke(
        self,
        *,
        call: Callable[[BindingT], Awaitable[NativeT]],
        fallback: Callable[[], Awaitable[ResultT]],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
        request_override: bool | None = None,
        eligible: bool = True,
    ) -> ResultT:
        result: Final = await self._aattempt(
            call=call,
            adapt=adapt,
            context=context,
            request_override=request_override,
            eligible=eligible,
        )
        if isinstance(result, RustHandled):
            return result.value
        return await fallback()

    def require(
        self,
        *,
        call: Callable[[BindingT], NativeT],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
        request_override: bool | None = None,
        eligible: bool = True,
    ) -> ResultT:
        result: Final = self._attempt(
            call=call,
            adapt=adapt,
            context=context,
            request_override=request_override,
            eligible=eligible,
        )
        if isinstance(result, RustHandled):
            return result.value
        self._raise_required(result)

    async def arequire(
        self,
        *,
        call: Callable[[BindingT], Awaitable[NativeT]],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
        request_override: bool | None = None,
        eligible: bool = True,
    ) -> ResultT:
        result: Final = await self._aattempt(
            call=call,
            adapt=adapt,
            context=context,
            request_override=request_override,
            eligible=eligible,
        )
        if isinstance(result, RustHandled):
            return result.value
        self._raise_required(result)

    def accepts(
        self,
        *,
        check: Callable[[BindingT], str | None],
        request_override: bool | None = None,
        eligible: bool = True,
    ) -> bool:
        binding_or_handoff: Final = self._binding_or_handoff(
            request_override=request_override,
            eligible=eligible,
        )
        if isinstance(binding_or_handoff, RustHandoff):
            return False
        try:
            reason: Final = check(binding_or_handoff)
        except Exception:  # noqa: BLE001  # preflight performs no provider I/O, so Python handoff is safe
            return False
        return reason is None

    def _binding_or_handoff(
        self,
        *,
        request_override: bool | None,
        eligible: bool,
    ) -> BindingT | RustHandoff:
        if not eligible or not self.enabled(request_override=request_override):
            return RustHandoff(HandoffReason.DISABLED)
        binding: Final = self.load()
        if binding is None:
            return RustHandoff(HandoffReason.UNAVAILABLE)
        return binding

    def _attempt_call(
        self,
        *,
        call: Callable[[], NativeT],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
    ) -> RustAttempt[ResultT]:
        exceptions: Final = native_exception_types()
        if exceptions is None:
            return RustHandled(adapt(call()))
        declined, upstream = exceptions
        try:
            value: Final = call()
        except declined as error:
            return RustHandoff(HandoffReason.DECLINED, _error_message(error))
        except upstream as error:
            self._raise_upstream(error, context)
        return RustHandled(adapt(value))

    async def _attempt_acall(
        self,
        *,
        call: Callable[[], Awaitable[NativeT]],
        adapt: Callable[[NativeT], ResultT],
        context: BridgeErrorContext,
    ) -> RustAttempt[ResultT]:
        exceptions: Final = native_exception_types()
        if exceptions is None:
            return RustHandled(adapt(await call()))
        declined, upstream = exceptions
        try:
            value: Final = await call()
        except declined as error:
            return RustHandoff(HandoffReason.DECLINED, _error_message(error))
        except upstream as error:
            self._raise_upstream(error, context)
        return RustHandled(adapt(value))

    def _raise_required(self, handoff: RustHandoff) -> NoReturn:
        detail: Final = f": {handoff.detail}" if handoff.detail else ""
        reason: Final = _required_reason(handoff.reason)
        raise RuntimeError(f"Rust {self.route} bridge {reason}{detail}")

    def _raise_upstream(self, error: BaseException, context: BridgeErrorContext) -> NoReturn:
        args: Final[tuple[object, ...]] = error.args
        attribute_status: Final = getattr(error, "status_code", None)
        attribute_message: Final = getattr(error, "message", None)
        status_value: Final = attribute_status if isinstance(attribute_status, int) else (args[0] if args else 0)
        message_value: Final = (
            attribute_message if isinstance(attribute_message, str) else (args[1] if len(args) > 1 else str(error))
        )
        status: Final = status_value if isinstance(status_value, int) else 0
        message: Final = message_value if isinstance(message_value, str) else str(message_value)
        raise APIError(
            status_code=status or 500,
            message=f"litellm rust {self.route}: {message}",
            llm_provider=context.provider,
            model=context.model,
        ) from error


def _error_message(error: BaseException) -> str:
    reason: Final[object] = error.args[0] if error.args else str(error)
    return reason if isinstance(reason, str) else str(reason)


def _required_reason(reason: HandoffReason) -> str:
    match reason:
        case HandoffReason.DISABLED:
            return "is disabled"
        case HandoffReason.UNAVAILABLE:
            return "is unavailable"
        case HandoffReason.DECLINED:
            return "declined the request"


def always_enabled(*, request_override: bool | None = None) -> bool:
    return True


def identity(value: ResultT) -> ResultT:
    return value


async def async_none() -> None:
    return None
