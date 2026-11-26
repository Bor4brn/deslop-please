export interface A {
  next?: B;
}

export interface B {
  next?: C;
}

export interface C {
  next?: D;
}

export interface D {
  next?: E;
}

export interface E {
  next?: F;
}

export interface F {
  next?: G;
}

export interface G {
  next?: H;
}

export interface H {
  next?: A;
}

