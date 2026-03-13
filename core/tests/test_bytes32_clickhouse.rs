use alloy::primitives::Bytes;
use rindexer::EthereumSqlTypeWrapper;

#[test]
fn test_bytes32_clickhouse_format() {
    // Create a bytes32 value (32 bytes)
    let bytes32_value = vec![
        0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd,
        0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab, 0xcd, 0xef, 0x12, 0x34, 0x56, 0x78, 0x90, 0xab,
        0xcd, 0xef,
    ];

    let bytes = Bytes::from(bytes32_value);
    let wrapper = EthereumSqlTypeWrapper::Bytes(bytes.clone());

    // Get the ClickHouse value format
    let clickhouse_value = wrapper.to_clickhouse_value();

    // Verify it's quoted (starts with ' and ends with ')
    assert!(
        clickhouse_value.starts_with('\'') && clickhouse_value.ends_with('\''),
        "Bytes value should be quoted for ClickHouse. Got: {}",
        clickhouse_value
    );

    // Verify it's in hex format (has 0x after the opening quote)
    assert!(
        clickhouse_value.starts_with("'0x"),
        "Bytes value should be in hex format with 0x prefix. Got: {}",
        clickhouse_value
    );

    // Expected format
    let expected = format!("'0x{}'", hex::encode(bytes));
    assert_eq!(
        clickhouse_value, expected,
        "Bytes value format mismatch.\nExpected: {}\nGot: {}",
        expected, clickhouse_value
    );

    println!("✅ Test passed! bytes32 is correctly formatted as: {}", clickhouse_value);
}

#[test]
fn test_bytes_nullable_clickhouse_format() {
    let bytes_value = vec![0xde, 0xad, 0xbe, 0xef];
    let bytes = Bytes::from(bytes_value);
    let wrapper = EthereumSqlTypeWrapper::BytesNullable(bytes.clone());

    let clickhouse_value = wrapper.to_clickhouse_value();

    // Verify it's quoted
    assert!(
        clickhouse_value.starts_with('\'') && clickhouse_value.ends_with('\''),
        "BytesNullable should be quoted. Got: {}",
        clickhouse_value
    );

    let expected = format!("'0x{}'", hex::encode(bytes));
    assert_eq!(clickhouse_value, expected);

    println!("✅ BytesNullable test passed! Format: {}", clickhouse_value);
}

#[test]
fn test_vec_bytes_clickhouse_format() {
    let bytes1 = Bytes::from(vec![0x12, 0x34]);
    let bytes2 = Bytes::from(vec![0x56, 0x78]);
    let wrapper = EthereumSqlTypeWrapper::VecBytes(vec![bytes1.clone(), bytes2.clone()]);

    let clickhouse_value = wrapper.to_clickhouse_value();

    // Verify it's an array with quoted elements
    assert!(clickhouse_value.starts_with('[') && clickhouse_value.ends_with(']'));
    assert!(clickhouse_value.contains("'0x1234'"));
    assert!(clickhouse_value.contains("'0x5678'"));

    println!("✅ VecBytes test passed! Format: {}", clickhouse_value);
}
