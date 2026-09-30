% covers: 12 - a set function refuses a cell holding an N-D char, which is no character vector however few its rows, rather than comparing it by its text with a row of the same characters; exit 1
unique({reshape('abcd', 1, 2, 2), 'abcd'})
