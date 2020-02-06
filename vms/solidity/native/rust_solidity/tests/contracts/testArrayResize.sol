pragma solidity ^0.4.10;

/**
 * Test array resize
 */
contract Test {
    uint8[] a;

    function f() {
        a.length = 20;
    }
}
