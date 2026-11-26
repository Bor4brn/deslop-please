from __future__ import annotations

from dataclasses import dataclass


@dataclass
class A:
    next: B | None


@dataclass
class B:
    next: C | None


@dataclass
class C:
    next: D | None


@dataclass
class D:
    next: E | None


@dataclass
class E:
    next: F | None


@dataclass
class F:
    next: G | None


@dataclass
class G:
    next: H | None


@dataclass
class H:
    next: A | None
