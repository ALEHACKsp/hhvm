<?hh // partial

class A {}

// ERROR
<<__Rx>>
function f(Mutable<A> $a): void {
}
