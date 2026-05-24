locals {
  resource_name = "memo-rs-s3-access"
}

data "aws_caller_identity" "current" {}

resource "aws_s3_bucket" "media" {
  bucket = var.s3_bucket_name
  tags   = var.tags
}

resource "aws_s3_bucket_public_access_block" "media" {
  bucket = aws_s3_bucket.media.id

  block_public_acls       = true
  block_public_policy     = true
  ignore_public_acls      = true
  restrict_public_buckets = true
}

resource "aws_s3_bucket_ownership_controls" "media" {
  bucket = aws_s3_bucket.media.id

  rule {
    object_ownership = "BucketOwnerEnforced"
  }
}

resource "aws_s3_bucket_cors_configuration" "media" {
  bucket = aws_s3_bucket.media.id

  cors_rule {
    allowed_headers = ["*"]
    allowed_methods = ["GET", "PUT", "HEAD"]
    allowed_origins = var.allowed_origins
    expose_headers  = ["ETag"]
    max_age_seconds = 3000
  }
}

data "aws_iam_policy_document" "app_s3_access" {
  statement {
    sid    = "AllowListBucket"
    effect = "Allow"

    actions = ["s3:ListBucket"]

    resources = [aws_s3_bucket.media.arn]
  }

  statement {
    sid    = "AllowObjectCrud"
    effect = "Allow"

    actions = [
      "s3:GetObject",
      "s3:PutObject",
      "s3:DeleteObject"
    ]

    resources = ["${aws_s3_bucket.media.arn}/*"]
  }
}

resource "aws_iam_policy" "app_s3_access" {
  name   = local.resource_name
  policy = data.aws_iam_policy_document.app_s3_access.json
  tags   = var.tags
}

data "aws_iam_policy_document" "app_role_assume" {
  statement {
    sid    = "AllowAssumeRole"
    effect = "Allow"

    actions = ["sts:AssumeRole"]

    principals {
      type = "AWS"
      identifiers = var.create_iam_user ? [
        aws_iam_user.app_user[0].arn
        ] : [
        "arn:aws:iam::${data.aws_caller_identity.current.account_id}:root"
      ]
    }
  }
}

resource "aws_iam_role" "app_s3_role" {
  name               = var.iam_role_name
  assume_role_policy = data.aws_iam_policy_document.app_role_assume.json
  tags               = var.tags
}

resource "aws_iam_role_policy_attachment" "app_s3_access" {
  role       = aws_iam_role.app_s3_role.name
  policy_arn = aws_iam_policy.app_s3_access.arn
}

resource "aws_iam_user" "app_user" {
  count = var.create_iam_user ? 1 : 0

  name = var.iam_username
  tags = var.tags
}

data "aws_iam_policy_document" "user_assume_role" {
  count = var.create_iam_user ? 1 : 0

  statement {
    sid    = "AllowAssumeAppRole"
    effect = "Allow"

    actions = ["sts:AssumeRole"]

    resources = [aws_iam_role.app_s3_role.arn]
  }
}

resource "aws_iam_user_policy" "user_assume_role" {
  count = var.create_iam_user ? 1 : 0

  name   = "memo-rs-assume-app-role"
  user   = aws_iam_user.app_user[0].name
  policy = data.aws_iam_policy_document.user_assume_role[0].json
}

resource "aws_iam_access_key" "app_user" {
  count = var.create_iam_access_key ? 1 : 0

  user = aws_iam_user.app_user[0].name
}

data "aws_iam_policy_document" "bucket_enforcement" {
  statement {
    sid    = "DenyInsecureTransport"
    effect = "Deny"

    principals {
      type        = "*"
      identifiers = ["*"]
    }

    actions = ["s3:*"]

    resources = [
      aws_s3_bucket.media.arn,
      "${aws_s3_bucket.media.arn}/*"
    ]

    condition {
      test     = "Bool"
      variable = "aws:SecureTransport"
      values   = ["false"]
    }
  }

  statement {
    sid    = "DenyGetPutWithoutPresignedUrl"
    effect = "Deny"

    principals {
      type        = "*"
      identifiers = ["*"]
    }

    actions = [
      "s3:GetObject",
      "s3:PutObject"
    ]

    resources = ["${aws_s3_bucket.media.arn}/*"]

    condition {
      test     = "StringNotEquals"
      variable = "s3:authType"
      values   = ["REST-QUERY-STRING"]
    }
  }

}

resource "aws_s3_bucket_policy" "media" {
  bucket = aws_s3_bucket.media.id
  policy = data.aws_iam_policy_document.bucket_enforcement.json
}
