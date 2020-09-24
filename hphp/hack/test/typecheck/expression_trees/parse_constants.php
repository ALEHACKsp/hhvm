<?hh

<<file:__EnableUnstableFeatures('expression_trees')>>

const int MY_CONST = 123;

class Code {
  const type TAst = mixed;
  // Simple literals.
  public function intLiteral(int $_): this::TAst {
    throw new Exception();
  }

  // Operators
  public function plus(this::TAst $_, this::TAst $_): this::TAst {
    throw new Exception();
  }

  // TODO: it would be better to discard unsupported syntax nodes during lowering.
  public function unsupportedSyntax(string $msg): this::TAst {
    throw new Exception($msg);
  }
}

function test(): void {
  $g = Code`1 + MY_CONST`;
}
