pragma solidity ^0.4.10;

contract stack {
    uint8[17*1024] list;

    function get_header() internal returns (uint) {
        return list[0];
    }

    function set_item(uint8 idx, uint8 value) {
        require(idx < 17*1024);
        list[idx] = value;
    }
}