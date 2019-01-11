contract Long {
    uint a = 0;
    function Long() {
        for (uint i = 0; i < 10000000000; i++) {
            a++;
        }
    }
}

contract LongCreator {
    function LongCreator() {
        Long a = new Long();
    }
}