resource "aws_dynamodb_table" "events" {
  name         = "events"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }

  ttl {
    attribute_name = "expire"
    enabled        = true
  }
}

resource "aws_dynamodb_table" "questions" {
  name         = "questions"
  billing_mode = "PAY_PER_REQUEST"
  hash_key     = "id"

  attribute {
    name = "id"
    type = "S"
  }

  attribute {
    name = "eid"
    type = "S"
  }

  ttl {
    attribute_name = "expire"
    enabled        = true
  }

  global_secondary_index {
    name               = "top"
    hash_key           = "eid"
    projection_type    = "INCLUDE"
    non_key_attributes = ["answered", "hidden", "votes"]
  }
}
