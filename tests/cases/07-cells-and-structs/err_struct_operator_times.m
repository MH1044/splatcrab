% covers: 15 - a binary operator on a struct is a clean error, exit 1, in the same sentence with the operator * and the class struct
% The spec's recorded text. disp(s.a) runs first; the Error: prefix names
% line 5, counting this covers line as line 1.
s.a = 1; disp(s.a)
s * 2
