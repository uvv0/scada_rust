        MODULE  lua_iar_jump

        SECTION .text:CODE:NOROOT(2)
        THUMB

        PUBLIC  lua_iar_setjmp
        PUBLIC  lua_iar_longjmp

lua_iar_setjmp
        STMIA   R0!, {R4-R11}
        MOV     R2, SP
        STMIA   R0!, {R2, LR}
        MOVS    R0, #0
        BX      LR

lua_iar_longjmp
        CMP     R1, #0
        BNE     lua_iar_longjmp_nonzero
        MOVS    R1, #1
lua_iar_longjmp_nonzero
        LDMIA   R0!, {R4-R11}
        LDMIA   R0!, {R2, LR}
        MOV     SP, R2
        MOV     R0, R1
        BX      LR

        END
