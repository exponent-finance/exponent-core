# DNum

Implementation of the `tcurve::num::Num` trait, used for calculating price curve math.

Why are we not using `fixed` as our number library? Because our price curve makes copious use of logarithms and exponentiation, which `fixed` does not support.

Why are we not using `rug` as our number library? Because it's big. So big. It has more firepower than needed.
