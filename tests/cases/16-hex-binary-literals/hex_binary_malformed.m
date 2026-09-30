% covers: 4 - a malformed literal is an invalid number naming the literal as written: no digit after the prefix, a digit outside the base, a letter that begins no suffix or a suffix not among the eight, and a value its type cannot hold; each through eval, caught and its message shown
texts = {
  'x = 0x;'
  'x = 0xu8;'
  'x = 0b102;'
  'x = 0x1Fz;'
  'x = 0x1Fu9;'
  'x = 0x1Fi;'
  'x = 0x100u8;'
  'x = 0x100s8;'
  'x = 0x10000000000000000;'
};
for k = 1:numel(texts)
  try
    eval(texts{k});
  catch e
    disp(e.message)
  end
end
