output "s3_bucket_name" {
  description = "Name of the media S3 bucket"
  value       = aws_s3_bucket.media.id
}

output "s3_bucket_arn" {
  description = "ARN of the media S3 bucket"
  value       = aws_s3_bucket.media.arn
}

output "s3_app_policy_arn" {
  description = "ARN of the IAM policy for app S3 access"
  value       = aws_iam_policy.app_s3_access.arn
}

output "iam_role_name" {
  description = "Name of the IAM role for S3 access"
  value       = aws_iam_role.app_s3_role.name
}

output "iam_role_arn" {
  description = "ARN of the IAM role for S3 access"
  value       = aws_iam_role.app_s3_role.arn
}

output "iam_username" {
  description = "Programmatic IAM username"
  value       = var.create_iam_user ? aws_iam_user.app_user[0].name : null
}

output "iam_access_key_id" {
  description = "Programmatic IAM access key ID"
  value       = var.create_iam_access_key ? aws_iam_access_key.app_user[0].id : null
}

output "iam_secret_access_key" {
  description = "Programmatic IAM secret access key"
  value       = var.create_iam_access_key ? aws_iam_access_key.app_user[0].secret : null
  sensitive   = true
}

output "aws_region" {
  description = "AWS region for deployed resources"
  value       = var.aws_region
}
