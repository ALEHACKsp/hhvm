<?hh

<<file:__EnableUnstableFeatures('expression_trees')>>

final class Code {
  const type TAst = mixed;
  // Simple literals.
  public function intLiteral(int $_): this::TAst {
    throw new Exception();
  }

  // Operators
  public function plus(this::TAst $_, this::TAst $_): this::TAst {
    throw new Exception();
  }

  public function splice(mixed $_): this::TAst {
    throw new Exception();
  }
}

function test(): void {
  Code`__splice__(1 + __splice__(4))`;
}
