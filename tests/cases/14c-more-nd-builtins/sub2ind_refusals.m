% covers: 10 - sub2ind refuses a subscript that is not a positive integer within its dimension's size (the folded size for the last, and 1 for a subscript past the end of sz), subscripts of different sizes that are not scalars, and a size that is not a row or column of positive integers; each refusal caught and its message shown
try
  sub2ind([2 3], 3, 1)
catch e
  disp(e.message)
end
try
  sub2ind([2 3], 1.5, 1)
catch e
  disp(e.message)
end
try
  sub2ind([2 3], 1, 4)
catch e
  disp(e.message)
end
try
  sub2ind([2 3 4], 1, 13)
catch e
  disp(e.message)
end
try
  sub2ind([2 3], 0, 1)
catch e
  disp(e.message)
end
try
  sub2ind([2 3], 1, 2, 2)
catch e
  disp(e.message)
end
try
  sub2ind([2 3], [1 2], [1 2 3])
catch e
  disp(e.message)
end
try
  sub2ind([2 3], [1 2], [1; 2])
catch e
  disp(e.message)
end
try
  sub2ind([2 0], 1, 1)
catch e
  disp(e.message)
end
try
  sub2ind([2 3.5], 1, 1)
catch e
  disp(e.message)
end
try
  sub2ind([2 3; 4 5], 1, 1)
catch e
  disp(e.message)
end
