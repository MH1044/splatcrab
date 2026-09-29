% covers: 4 - (Scope, strsplit and regexp) a cell they make past the byte budget is a clean error
% 3e7 pieces pass the element cap but cost more than 2 GB as cell elements.
x = strsplit(repmat(',', 1, 3e7), ',', 'CollapseDelimiters', false);
