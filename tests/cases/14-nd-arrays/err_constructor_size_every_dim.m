% covers: 2 - a constructor whose sizes multiply past the maximum array size is refused before anything is allocated, the message naming every size; exit 1, never an abort
zeros(1e5, 1e5, 1e5)
