MEMORY {
    BOOT2 : ORIGIN = 0x10000000, LENGTH = 0x100
    /* Keep the last two 4 KiB sectors for USB settings. */
    FLASH : ORIGIN = 0x10000100, LENGTH = 2040K - 0x100
    RAM   : ORIGIN = 0x20000000, LENGTH = 264K
}

SECTIONS {
    .boot2 ORIGIN(BOOT2) : {
        KEEP(*(.boot2))
    } > BOOT2
} INSERT BEFORE .vector_table;
