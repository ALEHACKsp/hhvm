<?hh // strict

class P {
  const T = 'string';
}

class C extends P {
  const type T = string;
}

<<__EntryPoint>> function main(): void {}
