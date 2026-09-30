% covers: 7 - circshift's shift must be a nonempty real row or column of integers, a fraction, a NaN, an empty, a matrix and a row holding a fraction each refused, and one integer when a dimension is given, a row or a column of two refused; each refusal caught and its message shown
try
  circshift(1:3, 1.5)
catch e
  disp(e.message)
end
try
  circshift(1:3, NaN)
catch e
  disp(e.message)
end
try
  circshift(1:3, [])
catch e
  disp(e.message)
end
try
  circshift(1:3, ones(2, 2))
catch e
  disp(e.message)
end
try
  circshift(1:3, [1 2.5])
catch e
  disp(e.message)
end
try
  circshift(1:3, [1 1], 2)
catch e
  disp(e.message)
end
try
  circshift(1:3, [1; 2], 1)
catch e
  disp(e.message)
end
