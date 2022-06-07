# Errata and optimizations for RFC6330

The following is a list of unofficial errata and potential optimizations, which I discovered while writing this library.

1) Section 5.4.2.2 says "There are at most L steps in the first phase". This should say, "at most L - P steps".
This is clear from the fact that the end condition is i + u = L, u is initialized to P, and i is incremented by 1
at each step.
2) Section 5.4.2.2 refers to choosing HDPC rows after non-HDPC rows. However, this can be strengthened to
"HDPC rows should never be chosen". This follows from the fact that at the end of the first phase, u rows, will
not have been chosen. Therefore if P >= H, HDPC rows will never be chosen, since the first phase will end before
they can be chosen.
Proof that P >= H:  
P >= H  
L - W >= H (substitute P)  
K' + S + H - W >= H (substitute L)  
K' + S >= W (subtract H and add W to both sides)  
K' + S >= W, can be verified empirically from the systematic constants in Table 2
3) Optimization: HDPC rows do not need to be copied into X. At the beginning of the second phase, X is truncated to be
i by i. It follows from errata 2 that none of these can be HDPC rows, since the same row and column swaps are performed
on X as on A.
4) Optimization: all non-HDPC rows have only the values zero and one, for the entire coding process. Proof:
    0) at construction the constraint matrix has only zeros and ones in non-HDPC rows
    1) during the first phase, only non-HDPC rows are added to other rows (see errata 2), since these contain only
    ones or zeros, and are used in elimination they will never be multiplied by a beta > 1, and therefore will never
    create a value > 1 in a non-HDPC row.
    2) at the beginning of the second phase, U_lower is the only part of the matrix which can have values > 1. This
    follows from the fact that: a) the submatrix V no longer exists. b) no HDPC rows can exist in the identity submatrix
    I (see errata 2). The second phase ends either in failure, or with U_lower equal to the identity matrix. During
    the second phase, no operations are performed on preceding U_lower. Therefore, at the end of the second
    phase A contains only binary values.
    3) the third phase does not introduce non-binary values, given that none already exist, because matrix
    multiplication over GF(256) with binary values cannot produce a non-binary value.
    4) the fourth phase does not introduce non-binary values unless one already exists. The variable "b"
    referenced in the specification can only be one, since the matrix is already binary.
    5) the fifth phase already cannot introduce non-binary values, because the matrix is already binary
5) Optimization: the matrix A is binary after the second phase. (see proof in errata 4)
6) Section 5.4.2.5 says "For each of the first i rows of U_upper, do the following: if the row has a nonzero
entry at position j, and if the value of that nonzero entry is b, then add to this row b times row j of I_u". This
should say "For each of the first i rows of U_upper, do the following: if the row has a one-valued entry at position
j, then add to this row, row j of I_u". This follows from the fact that all nonzero values are one (see errata 5).
7) Section 5.4.2.6 says:
    ```
    For j from 1 to i, perform the following operations:
       1.  If A[j,j] is not one, then divide row j of A by A[j,j].
       2.  For l from 1 to j-1, if A[j,l] is nonzero, then add A[j,l]
           multiplied with row l of A to row j of A.
    ```
   Because A is a binary matrix, this can be simplified to:
    ```
    For j from 1 to i, perform the following operations:
       For l from 1 to j-1, if A[j,l] is one, then add row l of A to row j of A.
    ```
8) Section 5.4.2.2 says "If r = 2 and there is no row with exactly 2 ones in V, then choose any row with exactly 2
nonzeros in V." It follows from Errata 2 & 4 that this can be ignored as it will never happen.
