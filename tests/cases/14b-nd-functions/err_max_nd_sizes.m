% covers: 3 - max of two N-D arrays whose sizes do not broadcast is refused with the operand-size message naming every dimension; exit 1
max(zeros(2, 3, 4), zeros(2, 3, 5))
